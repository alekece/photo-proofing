//! Asynchronous `lettre` SMTP transport configured from [`SmtpSettings`].

use std::time::Duration;

use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    transport::smtp::{
        authentication::Credentials,
        client::{Tls, TlsParameters},
    },
};
use pp_core::{
    config::{SmtpSettings, SmtpTlsMode},
    email::Email,
};

use crate::error::MailError;

const SMTP_TIMEOUT: Duration = Duration::from_secs(15);

/// Configured asynchronous SMTP transport shared across worker jobs.
#[derive(Clone)]
pub struct SmtpMailer {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from_email: Email,
    from_name: Option<String>,
}

impl SmtpMailer {
    pub fn new(settings: &SmtpSettings) -> Result<Self, MailError> {
        let mut builder = match settings.tls {
            SmtpTlsMode::Tls => {
                let tls = TlsParameters::new(settings.host.clone())?;

                AsyncSmtpTransport::<Tokio1Executor>::relay(&settings.host)?.tls(Tls::Wrapper(tls))
            }
            SmtpTlsMode::Starttls => {
                let tls = TlsParameters::new(settings.host.clone())?;

                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&settings.host)?
                    .tls(Tls::Required(tls))
            }
            SmtpTlsMode::None => {
                AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&settings.host)
            }
        };

        builder = builder.port(settings.port).timeout(Some(SMTP_TIMEOUT));

        if let Some((username, password)) = &settings.credentials {
            builder = builder.credentials(Credentials::new(
                username.clone(),
                password.as_str().to_owned(),
            ));
        }

        Ok(Self {
            transport: builder.build(),
            from_email: settings.from_email.clone(),
            from_name: settings.from_name.clone(),
        })
    }

    pub const fn sender_email(&self) -> &Email {
        &self.from_email
    }

    pub fn sender_name(&self) -> Option<&str> {
        self.from_name.as_deref()
    }

    pub async fn send(&self, message: Message) -> Result<(), MailError> {
        self.transport.send(message).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{error::Error, net::TcpListener, num::NonZeroUsize};

    use lettre::Message;
    use pp_core::{
        config::{SmtpSettings, SmtpTlsMode},
        email::Email,
    };

    use super::SmtpMailer;
    use crate::error::MailError;

    #[tokio::test]
    async fn mail_error_separates_display_from_source_and_preserves_concrete_cause() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let closed_port = listener.local_addr().unwrap().port();
        drop(listener);

        let mailer = SmtpMailer::new(&SmtpSettings {
            host: "127.0.0.1".to_owned(),
            port: closed_port,
            tls: SmtpTlsMode::None,
            credentials: None,
            from_email: "noreply@example.test".parse::<Email>().unwrap(),
            from_name: None,
            max_recipients_per_send: NonZeroUsize::new(25).unwrap(),
        })
        .unwrap();
        let message = Message::builder()
            .from("noreply@example.test".parse().unwrap())
            .to("client@example.test".parse().unwrap())
            .subject("Your gallery is ready")
            .body(String::from("Hello"))
            .unwrap();
        let error = mailer.send(message).await.unwrap_err();

        let MailError::Smtp(smtp_error) = &error else {
            panic!("expected MailError::Smtp, got {error:?}");
        };
        let _ = smtp_error.is_transient();
        let source = error.source().expect("MailError exposes underlying cause");

        assert!(
            !error.to_string().contains(&source.to_string()),
            "Display ({error}) must not duplicate source ({source})"
        );
    }
}
