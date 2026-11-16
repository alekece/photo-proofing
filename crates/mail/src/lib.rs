//! Pure Warm Paper multipart email rendering (`askama`) and SMTP delivery (`lettre`).

use std::{error::Error, time::Duration};

use askama::Template;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    address::AddressError,
    error::Error as LettreError,
    message::{
        Attachment, Body, Mailbox, MultiPart, SinglePart,
        header::{ContentType, ContentTypeErr},
    },
    transport::smtp::{
        Error as SmtpError,
        authentication::Credentials,
        client::{Tls, TlsParameters},
    },
};
use pp_core::{
    config::{SmtpSettings, SmtpTlsMode},
    email::{Email, EmailMessage, EmailSubject},
    types::{
        access_code::GalleryAccessCode,
        studio::{CoverSignature, FocalPoint, InvitationStage, Language},
    },
};
use thiserror::Error;
use time::Date;

const LOGO_CID: &str = "studio-logo@photoproofing";
const COVER_CID: &str = "gallery-cover@photoproofing";
const SMTP_TIMEOUT: Duration = Duration::from_secs(15);

type BoxError = Box<dyn Error + Send + Sync>;

/// Failure while constructing or delivering an outgoing email.
#[derive(Debug, Error)]
pub enum MailError {
    #[error("invalid email address")]
    Address(#[source] BoxError),
    #[error("invalid MIME content type")]
    ContentType(#[source] BoxError),
    #[error("failed to render email template")]
    Template(#[source] BoxError),
    #[error("failed to assemble MIME message")]
    Message(#[source] BoxError),
    #[error("SMTP delivery failed")]
    Smtp(#[source] BoxError),
}

impl From<AddressError> for MailError {
    fn from(error: AddressError) -> Self {
        Self::Address(Box::new(error))
    }
}

impl From<ContentTypeErr> for MailError {
    fn from(error: ContentTypeErr) -> Self {
        Self::ContentType(Box::new(error))
    }
}

impl From<askama::Error> for MailError {
    fn from(error: askama::Error) -> Self {
        Self::Template(Box::new(error))
    }
}

impl From<LettreError> for MailError {
    fn from(error: LettreError) -> Self {
        Self::Message(Box::new(error))
    }
}

impl From<SmtpError> for MailError {
    fn from(error: SmtpError) -> Self {
        Self::Smtp(Box::new(error))
    }
}

/// Which kind of Warm Paper email is being rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmailKind {
    Invitation(InvitationStage),
}

/// Optional inline JPEG cover attached to a Warm Paper card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverPhoto {
    pub jpeg: Vec<u8>,
    pub focal_point: FocalPoint,
}

/// Complete data required to build one Warm Paper multipart email message.
#[derive(Debug, Clone)]
pub struct WarmPaperEmail {
    pub from_email: Email,
    pub from_name: Option<String>,
    pub reply_to: Option<Email>,
    pub to_email: Email,
    pub to_name: Option<String>,
    pub subject: EmailSubject,
    pub studio_name: String,
    pub signature: CoverSignature,
    pub logo_png: Option<Vec<u8>>,
    pub cover: Option<CoverPhoto>,
    pub gallery_title: String,
    pub kind: EmailKind,
    pub language: Language,
    pub shoot_date: Option<Date>,
    pub photo_count: usize,
    pub message: EmailMessage,
    pub share_url: String,
    pub access_code: Option<GalleryAccessCode>,
    pub expires_at: Option<Date>,
}

/// Localized copy and formatting rules for one `(EmailKind, Language)` pair.
#[derive(Clone, Copy)]
struct EmailCopy {
    kind: EmailKind,
    language: Language,
}

impl EmailCopy {
    const fn lang_code(self) -> &'static str {
        match self.language {
            Language::En => "en",
            Language::Fr => "fr",
        }
    }

    const fn eyebrow(self) -> &'static str {
        match (self.kind, self.language) {
            (EmailKind::Invitation(InvitationStage::Proofing), Language::En) => "Proofing gallery",
            (EmailKind::Invitation(InvitationStage::Proofing), Language::Fr) => {
                "Galerie de sélection"
            }
            (EmailKind::Invitation(InvitationStage::Finals), Language::En) => "Final photos ready",
            (EmailKind::Invitation(InvitationStage::Finals), Language::Fr) => {
                "Photos retouchées prêtes"
            }
        }
    }

    const fn cta(self) -> &'static str {
        match (self.kind, self.language) {
            (EmailKind::Invitation(InvitationStage::Proofing), Language::En) => "View gallery",
            (EmailKind::Invitation(InvitationStage::Proofing), Language::Fr) => "Voir la galerie",
            (EmailKind::Invitation(InvitationStage::Finals), Language::En) => "View final photos",
            (EmailKind::Invitation(InvitationStage::Finals), Language::Fr) => {
                "Voir les photos retouchées"
            }
        }
    }

    const fn untitled_gallery(self) -> &'static str {
        match self.language {
            Language::En => "Untitled gallery",
            Language::Fr => "Galerie sans titre",
        }
    }

    const fn access_code_label(self) -> &'static str {
        match self.language {
            Language::En => "Access code",
            Language::Fr => "Code d’accès",
        }
    }

    fn format_long_date(self, date: Date) -> String {
        let month_index = usize::from(u8::from(date.month()).saturating_sub(1));
        let month = match self.language {
            Language::En => {
                const MONTHS: [&str; 12] = [
                    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov",
                    "Dec",
                ];

                MONTHS.get(month_index).copied().unwrap_or("Jan")
            }
            Language::Fr => {
                const MONTHS: [&str; 12] = [
                    "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.",
                    "oct.", "nov.", "déc.",
                ];

                MONTHS.get(month_index).copied().unwrap_or("janv.")
            }
        };

        format!("{} {month} {}", date.day(), date.year())
    }

    fn format_photo_count(self, count: usize) -> String {
        let singular = match self.language {
            Language::En => count == 1,
            Language::Fr => count <= 1,
        };

        if singular {
            format!("{count} photo")
        } else {
            format!("{count} photos")
        }
    }

    fn format_meta_line(self, shoot_date: Option<Date>, photo_count: usize) -> String {
        let count_text = self.format_photo_count(photo_count);

        match shoot_date {
            Some(date) => format!("{} · {count_text}", self.format_long_date(date)),
            None => count_text,
        }
    }

    fn format_expiry(self, date: Date) -> String {
        let formatted = self.format_long_date(date);

        match self.language {
            Language::En => format!("Available until {formatted}"),
            Language::Fr => format!("Disponible jusqu’au {formatted}"),
        }
    }
}

#[derive(Template)]
#[template(path = "warm_paper.html")]
struct WarmPaperHtmlTemplate<'a> {
    lang: &'static str,
    has_logo: bool,
    logo_cid: &'static str,
    logo_alt: &'a str,
    studio_header: Option<&'a str>,
    cover_cid: &'static str,
    cover_focal: Option<(u8, u8)>,
    eyebrow: &'static str,
    title: &'a str,
    meta_line: &'a str,
    message_lines: Vec<&'a str>,
    access_code_label: &'static str,
    access_code: Option<&'a str>,
    share_url: &'a str,
    cta: &'static str,
    expiry_line: Option<&'a str>,
}

#[derive(Template)]
#[template(path = "warm_paper.txt")]
struct WarmPaperTextTemplate<'a> {
    studio_header: Option<&'a str>,
    eyebrow: &'static str,
    title: &'a str,
    meta_line: &'a str,
    message: &'a str,
    access_code_label: &'static str,
    access_code: Option<&'a str>,
    cta: &'static str,
    share_url: &'a str,
    expiry_line: Option<&'a str>,
}

impl WarmPaperEmail {
    /// Sanitizes display names so CR/LF or control characters can never inject mail headers.
    fn sanitize_display_name(raw: Option<&str>) -> Option<String> {
        let raw = raw?;
        let cleaned: String = raw.chars().filter(|ch| !ch.is_control()).collect();
        let trimmed = cleaned.trim();

        if trimmed.is_empty() {
            return None;
        }

        Some(trimmed.to_owned())
    }

    fn mailbox(name: Option<&str>, email: &Email) -> Result<Mailbox, MailError> {
        Ok(Mailbox::new(
            Self::sanitize_display_name(name),
            email.as_str().parse()?,
        ))
    }

    fn effective_title(&self, copy: EmailCopy) -> &str {
        let trimmed = self.gallery_title.trim();

        if trimmed.is_empty() {
            copy.untitled_gallery()
        } else {
            trimmed
        }
    }

    /// Assembles the RFC 5322 / MIME `multipart/alternative` (and `multipart/related` when inline
    /// CID images are present) message ready for SMTP delivery.
    pub fn into_message(self) -> Result<Message, MailError> {
        let copy = EmailCopy {
            kind: self.kind,
            language: self.language,
        };
        let inline_logo = self
            .logo_png
            .as_ref()
            .filter(|_| self.signature.shows_logo())
            .cloned();
        let studio = self.studio_name.trim();
        let studio_header = (self.signature.shows_name() && !studio.is_empty()).then_some(studio);
        let logo_alt = if studio.is_empty() { "Studio" } else { studio };
        let title = self.effective_title(copy);
        let meta_line = copy.format_meta_line(self.shoot_date, self.photo_count);
        let expiry_line = self.expires_at.map(|date| copy.format_expiry(date));
        let access_code = self.access_code.as_ref().map(GalleryAccessCode::as_str);
        let cover_focal = self
            .cover
            .as_ref()
            .map(|cover| (cover.focal_point.x.get(), cover.focal_point.y.get()));
        let message_lines = self.message.as_str().split('\n').collect::<Vec<_>>();

        let plain = WarmPaperTextTemplate {
            studio_header,
            eyebrow: copy.eyebrow(),
            title,
            meta_line: &meta_line,
            message: self.message.as_str(),
            access_code_label: copy.access_code_label(),
            access_code,
            cta: copy.cta(),
            share_url: &self.share_url,
            expiry_line: expiry_line.as_deref(),
        }
        .render()?;
        let html = WarmPaperHtmlTemplate {
            lang: copy.lang_code(),
            has_logo: inline_logo.is_some(),
            logo_cid: LOGO_CID,
            logo_alt,
            studio_header,
            cover_cid: COVER_CID,
            cover_focal,
            eyebrow: copy.eyebrow(),
            title,
            meta_line: &meta_line,
            message_lines,
            access_code_label: copy.access_code_label(),
            access_code,
            share_url: &self.share_url,
            cta: copy.cta(),
            expiry_line: expiry_line.as_deref(),
        }
        .render()?;

        let mut builder = Message::builder()
            .from(Self::mailbox(self.from_name.as_deref(), &self.from_email)?)
            .to(Self::mailbox(self.to_name.as_deref(), &self.to_email)?)
            .subject(self.subject.as_str());

        if let Some(reply_to) = &self.reply_to {
            builder = builder.reply_to(Self::mailbox(None, reply_to)?);
        }

        let inline_cover = self.cover.map(|cover| cover.jpeg);

        let multipart = if inline_logo.is_some() || inline_cover.is_some() {
            let mut related = MultiPart::related().singlepart(SinglePart::html(html));

            if let Some(logo_bytes) = inline_logo {
                let logo_part = Attachment::new_inline(LOGO_CID.to_owned())
                    .body(Body::new(logo_bytes), ContentType::parse("image/png")?);

                related = related.singlepart(logo_part);
            }

            if let Some(cover_bytes) = inline_cover {
                let cover_part = Attachment::new_inline(COVER_CID.to_owned())
                    .body(Body::new(cover_bytes), ContentType::parse("image/jpeg")?);

                related = related.singlepart(cover_part);
            }

            MultiPart::alternative()
                .singlepart(SinglePart::plain(plain))
                .multipart(related)
        } else {
            MultiPart::alternative_plain_html(plain, html)
        };

        Ok(builder.multipart(multipart)?)
    }
}

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
    use std::{error::Error, num::NonZeroUsize};

    use pp_core::{
        config::{SmtpSettings, SmtpTlsMode},
        email::{Email, EmailMessage, EmailSubject},
        types::{
            access_code::GalleryAccessCode,
            studio::{CoverSignature, FocalCoordinate, FocalPoint, InvitationStage, Language},
        },
    };
    use time::macros::date;

    use super::{CoverPhoto, EmailKind, SmtpMailer, WarmPaperEmail};

    fn decode_quoted_printable(raw: &str) -> String {
        let unfolded = raw.replace("=\r\n", "");
        let bytes = unfolded.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut idx = 0;

        while idx < bytes.len() {
            if bytes[idx] == b'='
                && idx + 2 < bytes.len()
                && let Ok(hex) = std::str::from_utf8(&bytes[idx + 1..idx + 3])
                && let Ok(decoded) = u8::from_str_radix(hex, 16)
            {
                out.push(decoded);
                idx += 3;
                continue;
            }

            out.push(bytes[idx]);
            idx += 1;
        }

        String::from_utf8_lossy(&out).into_owned()
    }

    fn extract_part<'a>(decoded: &'a str, content_type: &str) -> &'a str {
        let (_, after_header) = decoded.split_once(content_type).expect("MIME part present");

        after_header.split("\r\n--").next().unwrap_or(after_header)
    }

    fn sample_email() -> WarmPaperEmail {
        WarmPaperEmail {
            from_email: "noreply@example.test".parse::<Email>().unwrap(),
            from_name: Some("Atelier Lumière".to_owned()),
            reply_to: Some("photographer@example.test".parse::<Email>().unwrap()),
            to_email: "client@example.test".parse::<Email>().unwrap(),
            to_name: Some("Clara".to_owned()),
            subject: "Your gallery is ready".parse::<EmailSubject>().unwrap(),
            studio_name: "Atelier Lumière".to_owned(),
            signature: CoverSignature::Name,
            logo_png: None,
            cover: None,
            gallery_title: "Clara & Julien".to_owned(),
            kind: EmailKind::Invitation(InvitationStage::Proofing),
            language: Language::En,
            shoot_date: Some(date!(2026 - 09 - 12)),
            photo_count: 24,
            message: "Thank you for a wonderful day."
                .parse::<EmailMessage>()
                .unwrap(),
            share_url: "https://photos.example.test/g/tok_123".to_owned(),
            access_code: Some("123456".parse::<GalleryAccessCode>().unwrap()),
            expires_at: Some(date!(2026 - 10 - 31)),
        }
    }

    #[test]
    fn warm_paper_html_and_plain_render_meta_line_and_place_access_code_above_cta() {
        let raw = String::from_utf8(sample_email().into_message().unwrap().formatted()).unwrap();
        let decoded = decode_quoted_printable(&raw);
        let plain = extract_part(&decoded, "Content-Type: text/plain");
        let html = extract_part(&decoded, "Content-Type: text/html");

        assert!(plain.contains("12 Sep 2026 · 24 photos"));
        assert!(html.contains("12 Sep 2026 · 24 photos"));
        assert!(plain.contains("Available until 31 Oct 2026"));
        assert!(html.contains("Available until 31 Oct 2026"));

        let plain_pin = plain.find("123456").expect("PIN in plain text");
        let plain_cta = plain
            .find("https://photos.example.test/g/tok_123")
            .expect("CTA in plain text");
        let html_pin = html.find("123456").expect("PIN in HTML");
        let html_cta = html
            .find("https://photos.example.test/g/tok_123")
            .expect("CTA in HTML");

        assert!(plain_pin < plain_cta, "PIN must precede CTA in plain text");
        assert!(html_pin < html_cta, "PIN must precede CTA in HTML");

        let mut fr_zero = sample_email();
        fr_zero.language = Language::Fr;
        fr_zero.kind = EmailKind::Invitation(InvitationStage::Finals);
        fr_zero.photo_count = 0;
        let decoded_fr_zero = decode_quoted_printable(
            &String::from_utf8(fr_zero.into_message().unwrap().formatted()).unwrap(),
        );

        assert!(decoded_fr_zero.contains("12 sept. 2026 · 0 photo"));
        assert!(!decoded_fr_zero.contains("0 photos"));
        assert!(decoded_fr_zero.contains("Disponible jusqu’au 31 oct. 2026"));
        assert!(decoded_fr_zero.contains("Photos retouchées prêtes"));
        assert!(decoded_fr_zero.contains("Voir les photos retouchées"));

        let mut fr_one = sample_email();
        fr_one.language = Language::Fr;
        fr_one.shoot_date = None;
        fr_one.photo_count = 1;
        fr_one.access_code = None;
        fr_one.expires_at = None;
        let decoded_fr_one = decode_quoted_printable(
            &String::from_utf8(fr_one.into_message().unwrap().formatted()).unwrap(),
        );
        let html_fr_one = extract_part(&decoded_fr_one, "Content-Type: text/html");

        assert!(html_fr_one.contains("1 photo"));
        assert!(!html_fr_one.contains("· 1 photo"));
        assert!(!decoded_fr_one.contains("Code d’accès"));
        assert!(!decoded_fr_one.contains("Disponible jusqu’au"));
    }

    #[test]
    fn warm_paper_escapes_untrusted_fields_while_preserving_message_line_breaks() {
        let mut email = sample_email();
        email.studio_name = "Studio <script>alert(1)</script>".to_owned();
        email.gallery_title = "Clara & Julien <img src=x onerror=alert(1)>".to_owned();
        email.message = "<script>alert(1)</script>\n\"A & B\""
            .parse::<EmailMessage>()
            .unwrap();

        let raw = String::from_utf8(email.into_message().unwrap().formatted()).unwrap();
        let decoded = decode_quoted_printable(&raw);
        let html = extract_part(&decoded, "Content-Type: text/html");

        assert!(!html.contains("<script>"));
        assert!(!html.contains("<img src=x"));
        assert!(!html.contains("&lt;br"));
        assert!(!html.contains("&#60;br"));
        assert!(
            html.contains("&#60;script&#62;alert(1)&#60;/script&#62;<br />&#34;A &#38; B&#34;")
        );
    }

    #[test]
    fn mailbox_strips_crlf_control_characters_from_display_names() {
        let mut email = sample_email();
        email.from_name = Some("Studio\r\nBcc: attacker@example.test".to_owned());
        email.to_name = Some("Alice\r\nCc: leak@example.test".to_owned());

        let raw = String::from_utf8(email.into_message().unwrap().formatted()).unwrap();

        assert!(!raw.contains("\r\nBcc:"));
        assert!(!raw.contains("\r\nCc:"));
        assert!(raw.contains("Reply-To: photographer@example.test"));
    }

    #[test]
    fn inline_logo_and_cover_attach_multipart_related_cids_respecting_signature() {
        let mut with_both = sample_email();
        with_both.signature = CoverSignature::Both;
        with_both.logo_png = Some(vec![0x89, b'P', b'N', b'G']);
        with_both.cover = Some(CoverPhoto {
            jpeg: vec![0xFF, 0xD8, 0xFF, 0xD9],
            focal_point: FocalPoint {
                x: FocalCoordinate::try_from(25).unwrap(),
                y: FocalCoordinate::try_from(75).unwrap(),
            },
        });

        let raw = String::from_utf8(with_both.into_message().unwrap().formatted()).unwrap();
        let decoded = decode_quoted_printable(&raw);
        let html = extract_part(&decoded, "Content-Type: text/html");

        assert!(html.contains("cid:studio-logo@photoproofing"));
        assert!(raw.contains("Content-ID: <studio-logo@photoproofing>"));
        assert!(html.contains("cid:gallery-cover@photoproofing"));
        assert!(raw.contains("Content-ID: <gallery-cover@photoproofing>"));
        assert!(html.contains("object-position:25% 75%"));

        let mut hidden_logo = sample_email();
        hidden_logo.signature = CoverSignature::None;
        hidden_logo.logo_png = Some(vec![0x89, b'P', b'N', b'G']);

        let raw_hidden =
            String::from_utf8(hidden_logo.into_message().unwrap().formatted()).unwrap();
        let decoded_hidden = decode_quoted_printable(&raw_hidden);

        assert!(!decoded_hidden.contains("cid:studio-logo@photoproofing"));
        assert!(!raw_hidden.contains("Content-ID: <studio-logo@photoproofing>"));
    }

    #[tokio::test]
    async fn mail_error_separates_display_from_source() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
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
        let message = sample_email().into_message().unwrap();
        let error = mailer.send(message).await.unwrap_err();
        let source = error.source().expect("MailError exposes underlying cause");

        assert!(
            !error.to_string().contains(&source.to_string()),
            "Display ({error}) must not duplicate source ({source})"
        );
    }
}
