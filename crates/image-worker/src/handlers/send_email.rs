//! Sends queued gallery invitation and delivery emails.

use std::sync::Arc;

use pp_core::{types::access_code::AccessCodeCipher, url::Origin};
use pp_database::{
    Database,
    auth::AuthRepository,
    contact::ContactRepository,
    gallery::{GalleryRepository, ShareToken},
    jobs::{EmailRecipient, SendEmail},
    photo::PhotoId,
    settings::SettingsRepository,
};
use pp_storage::{PhotoKey, PhotoStorage, StorageError, Variant};
use thiserror::Error;
use tokio::io::AsyncReadExt;
use tracing::info;

use crate::{
    handlers::HandlerError,
    mail::{CoverPhoto, SmtpMailer, WarmPaperEmail},
};

#[derive(Debug, Error)]
enum SendEmailError {
    #[error("SMTP is not configured on the worker")]
    SmtpNotConfigured,
    #[error("gallery password key is required to decrypt gallery access code")]
    MissingPasswordKey,
    #[error("gallery access code could not be decrypted")]
    InvalidAccessCodeCiphertext,
}

/// Resolves live gallery/contact state and delivers one [`SendEmail`] job over SMTP.
pub struct SendEmailHandler {
    database: Database,
    storage: PhotoStorage,
    app_origin: Origin,
    cipher: Option<AccessCodeCipher>,
    mailer: Option<SmtpMailer>,
}

impl SendEmailHandler {
    pub const fn new(
        database: Database,
        storage: PhotoStorage,
        app_origin: Origin,
        cipher: Option<AccessCodeCipher>,
        mailer: Option<SmtpMailer>,
    ) -> Self {
        Self {
            database,
            storage,
            app_origin,
            cipher,
            mailer,
        }
    }

    async fn read_cover_jpeg(
        &self,
        location: pp_database::photo::PhotoLocation,
        hero_id: PhotoId,
    ) -> Result<Option<Vec<u8>>, StorageError> {
        let key = PhotoKey::with_location(location, hero_id, Variant::Thumbnail);

        match self.storage.read(&key).await {
            Ok(mut file) => {
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes).await?;

                Ok(Some(bytes))
            }
            Err(StorageError::NotFound) => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub async fn run(self: Arc<Self>, payload: SendEmail) -> Result<(), HandlerError> {
        let Some(mailer) = &self.mailer else {
            return Err(HandlerError::failed(SendEmailError::SmtpNotConfigured));
        };

        let (to_email, to_name, share_token): (_, Option<String>, ShareToken) =
            match payload.recipient {
                EmailRecipient::Contact { contact_id } => {
                    let Some(link) = self
                        .database
                        .repository::<ContactRepository>()
                        .find_gallery_link(payload.workspace_id, payload.gallery_id, contact_id)
                        .await
                        .map_err(HandlerError::failed)?
                    else {
                        return Ok(());
                    };

                    (
                        link.contact.email,
                        Some(link.contact.name.as_str().to_owned()),
                        link.share_token,
                    )
                }
                EmailRecipient::Bcc { email } => {
                    let Some(gallery) = self
                        .database
                        .repository::<GalleryRepository>()
                        .get(payload.workspace_id, payload.gallery_id)
                        .await
                        .map_err(HandlerError::failed)?
                    else {
                        return Ok(());
                    };

                    let Some(share_token) = gallery.share_token else {
                        return Ok(());
                    };

                    (email, None, share_token)
                }
            };

        let Some(shared) = self
            .database
            .repository::<GalleryRepository>()
            .find_shared(&share_token)
            .await
            .map_err(HandlerError::failed)?
        else {
            return Ok(());
        };

        if shared.location.workspace_id != payload.workspace_id
            || shared.location.gallery_id != payload.gallery_id
        {
            return Ok(());
        }

        let access_code = match &shared.access_code_ciphertext {
            Some(ciphertext) => {
                let cipher = self
                    .cipher
                    .as_ref()
                    .ok_or_else(|| HandlerError::failed(SendEmailError::MissingPasswordKey))?;
                let code = cipher
                    .decrypt(payload.gallery_id, ciphertext)
                    .map_err(|_| {
                        HandlerError::failed(SendEmailError::InvalidAccessCodeCiphertext)
                    })?;

                Some(code)
            }
            None => None,
        };

        let settings_repo = self.database.repository::<SettingsRepository>();
        let settings = settings_repo
            .settings(payload.workspace_id)
            .await
            .map_err(HandlerError::failed)?;
        let language = settings_repo
            .language(payload.workspace_id)
            .await
            .map_err(HandlerError::failed)?;
        let reply_to = self
            .database
            .repository::<AuthRepository>()
            .account_email(payload.workspace_id)
            .await
            .map_err(HandlerError::failed)?;

        let cover = match shared.gallery.hero_id {
            Some(hero_id) => self
                .read_cover_jpeg(shared.location, hero_id)
                .await
                .map_err(HandlerError::failed)?
                .map(|jpeg| CoverPhoto {
                    jpeg,
                    focal_point: shared.gallery.focal_point,
                }),
            None => None,
        };

        let from_name = mailer.from_name().map(str::to_owned).or_else(|| {
            let trimmed = settings.studio_name.trim();

            (!trimmed.is_empty()).then(|| trimmed.to_owned())
        });
        let share_url = format!("{}/g/{}", self.app_origin.as_str(), share_token.as_str());

        let email = WarmPaperEmail {
            from_email: mailer.from_email().clone(),
            from_name,
            reply_to,
            to_email,
            to_name,
            subject: payload.subject,
            studio_name: settings.studio_name,
            signature: settings.signature,
            logo_png: settings.logo,
            cover,
            gallery_title: shared.gallery.title.unwrap_or_default(),
            stage: payload.stage,
            language,
            message: payload.message,
            share_url,
            access_code,
            expires_at: shared.gallery.expires_at,
        };

        let message = email.into_message().map_err(HandlerError::failed)?;
        mailer.send(message).await.map_err(HandlerError::failed)?;
        info!(
            gallery = %payload.gallery_id.0,
            stage = ?payload.stage,
            "sent gallery invitation email"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        num::NonZeroUsize,
        sync::Arc,
        time::{Duration, SystemTime},
    };

    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    use pp_core::{
        config::{PhotoDir, SmtpSettings, SmtpTlsMode},
        email::{Email, EmailMessage, EmailSubject},
        types::{
            access_code::{AccessCodeCipher, EncryptionKey, GalleryAccessCode},
            generate::Generate,
            timestamp::Timestamp,
        },
        url::Origin,
    };
    use pp_database::{
        Database, PgPool,
        auth::AuthRepository,
        contact::ContactRepository,
        gallery::{GalleryChanges, GalleryRepository, InvitationBatch, InvitationContact},
        jobs::{JobKind, JobOptions, JobRepository, SendEmail},
        photo::{PhotoId, PhotoLocation},
        settings::{CoverSignature, LogoChange, SettingsRepository, SettingsUpdate},
        tests::TestRepository,
    };
    use pp_storage::{PhotoKey, PhotoStorage, Variant};
    use serde_json::from_str;
    use tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        net::TcpListener,
        sync::Mutex,
    };

    use super::SendEmailHandler;
    use crate::{
        handlers::{CompleteJob, Finalizer},
        mail::SmtpMailer,
    };

    #[derive(Debug, Clone)]
    struct CapturedMail {
        mail_from: String,
        rcpt_to: Vec<String>,
        data: String,
    }

    struct MockSmtp {
        port: u16,
        messages: Arc<Mutex<Vec<CapturedMail>>>,
    }

    impl MockSmtp {
        async fn start() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let messages = Arc::new(Mutex::new(Vec::new()));
            let captured = Arc::clone(&messages);

            tokio::spawn(async move {
                while let Ok((stream, _)) = listener.accept().await {
                    let captured = Arc::clone(&captured);
                    tokio::spawn(async move {
                        let (reader, mut writer) = stream.into_split();
                        let mut lines = BufReader::new(reader).lines();
                        let _ = writer.write_all(b"220 localhost ESMTP\r\n").await;
                        let mut mail_from = String::new();
                        let mut rcpt_to = Vec::new();
                        let mut in_data = false;
                        let mut data = String::new();

                        while let Ok(Some(line)) = lines.next_line().await {
                            if in_data {
                                if line == "." {
                                    in_data = false;
                                    captured.lock().await.push(CapturedMail {
                                        mail_from: std::mem::take(&mut mail_from),
                                        rcpt_to: std::mem::take(&mut rcpt_to),
                                        data: std::mem::take(&mut data),
                                    });
                                    let _ = writer.write_all(b"250 2.0.0 Queued\r\n").await;
                                } else {
                                    let unstuffed = line.strip_prefix('.').unwrap_or(&line);
                                    data.push_str(unstuffed);
                                    data.push_str("\r\n");
                                }
                                continue;
                            }

                            let upper = line.to_ascii_uppercase();
                            if upper.starts_with("EHLO") || upper.starts_with("HELO") {
                                let _ =
                                    writer.write_all(b"250-localhost\r\n250 8BITMIME\r\n").await;
                            } else if upper.starts_with("MAIL FROM:") {
                                mail_from = line;
                                let _ = writer.write_all(b"250 2.1.0 OK\r\n").await;
                            } else if upper.starts_with("RCPT TO:") {
                                rcpt_to.push(line);
                                let _ = writer.write_all(b"250 2.1.5 OK\r\n").await;
                            } else if upper == "DATA" {
                                in_data = true;
                                let _ = writer.write_all(b"354 Start mail input\r\n").await;
                            } else if upper == "QUIT" {
                                let _ = writer.write_all(b"221 2.0.0 Bye\r\n").await;
                                break;
                            } else {
                                let _ = writer.write_all(b"250 OK\r\n").await;
                            }
                        }
                    });
                }
            });

            Self { port, messages }
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn sends_per_contact_and_bcc_invitations_with_decrypted_access_code_and_cover(
        pool: PgPool,
    ) {
        let smtp_server = MockSmtp::start().await;
        let dir = tempfile::tempdir().unwrap();
        let storage = PhotoStorage::open(&PhotoDir::try_from(dir.path().to_owned()).unwrap())
            .await
            .unwrap();
        let database = Database::new(pool.clone());
        let identity = database
            .repository::<AuthRepository>()
            .create_account(
                &"studio@example.test".parse::<Email>().unwrap(),
                "hash",
                &[1; 32],
                SystemTime::now() + Duration::from_secs(3600),
            )
            .await
            .unwrap()
            .unwrap();
        let workspace_id = identity.workspace_id;

        let current_settings = database
            .repository::<SettingsRepository>()
            .settings(workspace_id)
            .await
            .unwrap();
        database
            .repository::<SettingsRepository>()
            .save(
                workspace_id,
                SettingsUpdate {
                    studio_name: "Studio Lumière".to_owned(),
                    mark: current_settings.mark,
                    pattern: current_settings.pattern,
                    position: current_settings.position,
                    size: current_settings.size,
                    signature: CoverSignature::Name,
                    signature_size: current_settings.signature_size,
                    proof_quality: current_settings.proof_quality,
                    logo: LogoChange::Keep,
                },
            )
            .await
            .unwrap();

        let key: EncryptionKey = URL_SAFE_NO_PAD.encode([7_u8; 32]).parse().unwrap();
        let cipher = AccessCodeCipher::new(&key);
        let gallery = database
            .repository::<GalleryRepository>()
            .create(workspace_id)
            .await
            .unwrap();
        let code: GalleryAccessCode = "739104".parse().unwrap();
        let ciphertext = cipher.encrypt(gallery.id, &code).unwrap();
        let location = PhotoLocation {
            workspace_id,
            gallery_id: gallery.id,
        };
        let photo_id = PhotoId::generate();
        sqlx::query(
            "INSERT INTO photos (id, workspace_id, gallery_id, original_name, size, width, height,
                                 status, clean_ready)
             VALUES ($1, $2, $3, 'cover.jpg', 5, 2, 2, 'ready', TRUE)",
        )
        .bind(photo_id)
        .bind(workspace_id)
        .bind(gallery.id)
        .execute(&pool)
        .await
        .unwrap();

        database
            .repository::<GalleryRepository>()
            .update(
                workspace_id,
                gallery.id,
                &GalleryChanges {
                    title: Some(Some("Clara & Julien".to_owned())),
                    access_code: Some(Some(ciphertext)),
                    hero_id: Some(Some(photo_id)),
                    ..GalleryChanges::default()
                },
            )
            .await
            .unwrap();

        let mut staged = storage.stage().await.unwrap();
        staged.write(&[0xFF, 0xD8, 0xFF, 0xD9]).await.unwrap();
        staged
            .commit(&PhotoKey::with_location(
                location,
                photo_id,
                Variant::Thumbnail,
            ))
            .await
            .unwrap();

        let queued = database
            .repository::<GalleryRepository>()
            .queue_invitations(
                workspace_id,
                gallery.id,
                &InvitationBatch {
                    subject: "Clara & Julien — Proofing".parse::<EmailSubject>().unwrap(),
                    message: "Please pick your favorites."
                        .parse::<EmailMessage>()
                        .unwrap(),
                    to: vec![InvitationContact {
                        email: "alice@example.test".parse::<Email>().unwrap(),
                        name: None,
                    }],
                    bcc: vec!["archive@example.test".parse::<Email>().unwrap()],
                },
                JobOptions::default(),
            )
            .await
            .unwrap()
            .unwrap();

        let contacts = database
            .repository::<ContactRepository>()
            .list_for_gallery(workspace_id, gallery.id)
            .await
            .unwrap()
            .unwrap();
        let general_token = queued.gallery.share_token.unwrap();
        let alice_link = database
            .repository::<ContactRepository>()
            .find_gallery_link(workspace_id, gallery.id, contacts[0].id)
            .await
            .unwrap()
            .unwrap();
        let alice_token = alice_link.share_token;

        let mailer = SmtpMailer::new(&SmtpSettings {
            host: "127.0.0.1".to_owned(),
            port: smtp_server.port,
            tls: SmtpTlsMode::None,
            credentials: None,
            from_email: "noreply@example.test".parse::<Email>().unwrap(),
            from_name: Some("Photo Proofing".to_owned()),
            max_recipients_per_send: NonZeroUsize::new(25).unwrap(),
        })
        .unwrap();

        let handler = Arc::new(SendEmailHandler::new(
            database.clone(),
            storage,
            "https://photos.example.test".parse::<Origin>().unwrap(),
            Some(cipher),
            Some(mailer),
        ));
        let finalizer = CompleteJob::new(database.clone());

        for _ in 0..2 {
            let job = database
                .repository::<JobRepository>()
                .claim(&[JobKind::SendEmail], std::time::Duration::from_secs(60))
                .await
                .unwrap()
                .unwrap();
            let payload: SendEmail = job.payload().unwrap();
            Arc::clone(&handler).run(payload).await.unwrap();
            let completion = finalizer.finalize(&job, ()).await.unwrap();
            assert!(completion.is_current());
        }

        let sent = smtp_server.messages.lock().await;
        assert_eq!(sent.len(), 2);

        let alice_mail = sent
            .iter()
            .find(|m| m.rcpt_to.iter().any(|r| r.contains("alice@example.test")))
            .unwrap();
        assert!(alice_mail.mail_from.contains("noreply@example.test"));
        let decoded_alice = alice_mail.data.replace("=\r\n", "").replace("=3D", "=");
        assert!(decoded_alice.contains(&format!(
            "https://photos.example.test/g/{}",
            alice_token.as_str()
        )));
        assert!(!decoded_alice.contains(general_token.as_str()));
        assert!(!decoded_alice.contains("archive@example.test"));
        assert!(decoded_alice.contains("739104"));
        assert!(alice_mail.data.contains("Reply-To: studio@example.test"));
        assert!(
            alice_mail
                .data
                .contains("Content-ID: <gallery-cover@photoproofing>")
        );

        let bcc_mail = sent
            .iter()
            .find(|m| m.rcpt_to.iter().any(|r| r.contains("archive@example.test")))
            .unwrap();
        let decoded_bcc = bcc_mail.data.replace("=\r\n", "").replace("=3D", "=");
        assert!(decoded_bcc.contains(&format!(
            "https://photos.example.test/g/{}",
            general_token.as_str()
        )));
        assert!(!decoded_bcc.contains(alice_token.as_str()));
        assert!(!decoded_bcc.contains("alice@example.test"));
        assert!(decoded_bcc.contains("739104"));
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn skips_archived_expired_or_unlinked_targets_and_fails_without_smtp_or_cipher(
        pool: PgPool,
    ) {
        let smtp_server = MockSmtp::start().await;
        let dir = tempfile::tempdir().unwrap();
        let storage = PhotoStorage::open(&PhotoDir::try_from(dir.path().to_owned()).unwrap())
            .await
            .unwrap();
        let database = Database::new(pool);
        let workspace_id = database
            .repository::<TestRepository>()
            .create_workspace()
            .await
            .unwrap();
        let key: EncryptionKey = URL_SAFE_NO_PAD.encode([9_u8; 32]).parse().unwrap();
        let cipher = AccessCodeCipher::new(&key);

        let gallery = database
            .repository::<GalleryRepository>()
            .create(workspace_id)
            .await
            .unwrap();
        let code: GalleryAccessCode = "112233".parse().unwrap();
        database
            .repository::<GalleryRepository>()
            .update(
                workspace_id,
                gallery.id,
                &GalleryChanges {
                    access_code: Some(Some(cipher.encrypt(gallery.id, &code).unwrap())),
                    ..GalleryChanges::default()
                },
            )
            .await
            .unwrap();

        database
            .repository::<GalleryRepository>()
            .queue_invitations(
                workspace_id,
                gallery.id,
                &InvitationBatch {
                    subject: "Invite".parse::<EmailSubject>().unwrap(),
                    message: "Welcome".parse::<EmailMessage>().unwrap(),
                    to: vec![InvitationContact {
                        email: "bob@example.test".parse::<Email>().unwrap(),
                        name: None,
                    }],
                    bcc: vec![],
                },
                JobOptions::default(),
            )
            .await
            .unwrap()
            .unwrap();

        let job = database
            .repository::<JobRepository>()
            .claim(&[JobKind::SendEmail], std::time::Duration::from_secs(60))
            .await
            .unwrap()
            .unwrap();
        let payload: SendEmail = job.payload().unwrap();

        // 1. Without SMTP mailer -> fails recoverably
        let no_smtp = Arc::new(SendEmailHandler::new(
            database.clone(),
            storage.clone(),
            "https://photos.example.test".parse::<Origin>().unwrap(),
            Some(cipher.clone()),
            None,
        ));
        assert!(no_smtp.run(payload.clone()).await.is_err());

        let mailer = SmtpMailer::new(&SmtpSettings {
            host: "127.0.0.1".to_owned(),
            port: smtp_server.port,
            tls: SmtpTlsMode::None,
            credentials: None,
            from_email: "noreply@example.test".parse::<Email>().unwrap(),
            from_name: None,
            max_recipients_per_send: NonZeroUsize::new(25).unwrap(),
        })
        .unwrap();

        // 2. Password-protected gallery without cipher -> fails rather than omitting code
        let no_cipher = Arc::new(SendEmailHandler::new(
            database.clone(),
            storage.clone(),
            "https://photos.example.test".parse::<Origin>().unwrap(),
            None,
            Some(mailer.clone()),
        ));
        assert!(no_cipher.run(payload.clone()).await.is_err());

        let handler = Arc::new(SendEmailHandler::new(
            database.clone(),
            storage,
            "https://photos.example.test".parse::<Origin>().unwrap(),
            Some(cipher),
            Some(mailer),
        ));

        // 3. Unlinked contact -> clean no-op, zero emails sent
        let contacts = database
            .repository::<ContactRepository>()
            .list_for_gallery(workspace_id, gallery.id)
            .await
            .unwrap()
            .unwrap();
        database
            .repository::<ContactRepository>()
            .unlink_gallery(workspace_id, gallery.id, contacts[0].id)
            .await
            .unwrap();
        Arc::clone(&handler).run(payload.clone()).await.unwrap();
        assert!(smtp_server.messages.lock().await.is_empty());

        // 4. Re-link contact, then expire gallery -> clean no-op, zero emails sent
        database
            .repository::<ContactRepository>()
            .link_gallery(workspace_id, gallery.id, contacts[0].id)
            .await
            .unwrap();
        database
            .repository::<GalleryRepository>()
            .update(
                workspace_id,
                gallery.id,
                &GalleryChanges {
                    expires_at: Some(Some(
                        from_str::<Timestamp>("\"2020-01-01T00:00:00Z\"").unwrap(),
                    )),
                    ..GalleryChanges::default()
                },
            )
            .await
            .unwrap();
        Arc::clone(&handler).run(payload).await.unwrap();
        assert!(smtp_server.messages.lock().await.is_empty());
    }
}
