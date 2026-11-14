//! Warm Paper multipart email rendering and SMTP transport.

use std::{fmt, time::Duration};

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
    types::{access_code::GalleryAccessCode, timestamp::Timestamp},
};
use pp_database::{
    gallery::FocalPoint,
    jobs::InvitationStage,
    settings::{CoverSignature, Language},
};
use thiserror::Error;

const LOGO_CID: &str = "studio-logo@photoproofing";
const COVER_CID: &str = "gallery-cover@photoproofing";
const SMTP_TIMEOUT: Duration = Duration::from_secs(15);

/// Failure while constructing or delivering an outgoing email.
#[derive(Debug, Error)]
pub enum MailError {
    #[error("invalid email address: {0}")]
    Address(#[from] AddressError),
    #[error("invalid MIME content type: {0}")]
    ContentType(#[from] ContentTypeErr),
    #[error("failed to assemble MIME message: {0}")]
    Message(#[from] LettreError),
    #[error("SMTP error: {0}")]
    Smtp(#[from] SmtpError),
}

/// Optional inline JPEG cover attached to a Warm Paper invitation card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverPhoto {
    pub jpeg: Vec<u8>,
    pub focal_point: FocalPoint,
}

/// Escapes untrusted text for HTML element bodies and double-quoted attributes.
struct HtmlEscaped<'a>(&'a str);

impl fmt::Display for HtmlEscaped<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for ch in self.0.chars() {
            match ch {
                '&' => formatter.write_str("&amp;")?,
                '<' => formatter.write_str("&lt;")?,
                '>' => formatter.write_str("&gt;")?,
                '"' => formatter.write_str("&quot;")?,
                '\'' => formatter.write_str("&#39;")?,
                other => fmt::Write::write_char(formatter, other)?,
            }
        }

        Ok(())
    }
}

///Localized static copy for one `(InvitationStage, Language)` pair.
#[derive(Clone, Copy)]
struct EmailCopy {
    stage: InvitationStage,
    language: Language,
}

impl EmailCopy {
    const fn eyebrow(self) -> &'static str {
        match (self.stage, self.language) {
            (InvitationStage::Proofing, Language::En) => "Proofing gallery",
            (InvitationStage::Proofing, Language::Fr) => "Galerie de sélection",
            (InvitationStage::Finals, Language::En) => "Final photos ready",
            (InvitationStage::Finals, Language::Fr) => "Photos retouchées prêtes",
        }
    }

    const fn cta(self) -> &'static str {
        match (self.stage, self.language) {
            (InvitationStage::Proofing, Language::En) => "View gallery",
            (InvitationStage::Proofing, Language::Fr) => "Voir la galerie",
            (InvitationStage::Finals, Language::En) => "View final photos",
            (InvitationStage::Finals, Language::Fr) => "Voir les photos retouchées",
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

    fn format_expiry(self, timestamp: Timestamp) -> String {
        let instant = timestamp.0;
        let year = instant.year();
        let day = instant.day();
        let month_index = usize::from(u8::from(instant.month()).saturating_sub(1));

        match self.language {
            Language::En => {
                const MONTHS: [&str; 12] = [
                    "January",
                    "February",
                    "March",
                    "April",
                    "May",
                    "June",
                    "July",
                    "August",
                    "September",
                    "October",
                    "November",
                    "December",
                ];
                let month = MONTHS.get(month_index).copied().unwrap_or("January");

                format!("Available until {month} {day}, {year}")
            }
            Language::Fr => {
                const MONTHS: [&str; 12] = [
                    "janvier",
                    "février",
                    "mars",
                    "avril",
                    "mai",
                    "juin",
                    "juillet",
                    "août",
                    "septembre",
                    "octobre",
                    "novembre",
                    "décembre",
                ];
                let month = MONTHS.get(month_index).copied().unwrap_or("janvier");

                format!("Disponible jusqu’au {day} {month} {year}")
            }
        }
    }
}

/// Complete data required to build one Warm Paper multipart invitation message.
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
    pub stage: InvitationStage,
    pub language: Language,
    pub message: EmailMessage,
    pub share_url: String,
    pub access_code: Option<GalleryAccessCode>,
    pub expires_at: Option<Timestamp>,
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

    fn render_plain(&self, copy: EmailCopy) -> String {
        let mut sections = Vec::new();
        let studio = self.studio_name.trim();

        if self.signature.shows_name() && !studio.is_empty() {
            sections.push(studio.to_owned());
        }

        sections.push(format!(
            "{}\n{}",
            copy.eyebrow(),
            self.effective_title(copy)
        ));
        sections.push(self.message.as_str().to_owned());
        sections.push(format!("{}: {}", copy.cta(), self.share_url));

        if let Some(code) = &self.access_code {
            sections.push(format!("{}: {}", copy.access_code_label(), code.as_str()));
        }

        if let Some(expires_at) = self.expires_at {
            sections.push(copy.format_expiry(expires_at));
        }

        sections.join("\n\n")
    }

    fn render_html(&self, copy: EmailCopy, has_logo: bool) -> String {
        let title = self.effective_title(copy);
        let studio = self.studio_name.trim();
        let shows_name = self.signature.shows_name() && !studio.is_empty();
        let mut signature_html = String::new();

        if has_logo || shows_name {
            signature_html
                .push_str(r#"<tr><td align="center" style="padding:28px 32px 12px 32px;">"#);

            if has_logo {
                let alt = if studio.is_empty() { "Studio" } else { studio };
                signature_html.push_str(&format!(
                    r#"<img src="cid:{LOGO_CID}" alt="{}" style="display:block;max-height:44px;max-width:180px;width:auto;height:auto;margin:0 auto 8px auto;border:0;" />"#,
                    HtmlEscaped(alt)
                ));
            }

            if shows_name {
                signature_html.push_str(&format!(
                    r#"<div style="font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;font-size:12px;letter-spacing:0.14em;text-transform:uppercase;color:#57534e;">{}</div>"#,
                    HtmlEscaped(studio)
                ));
            }

            signature_html.push_str("</td></tr>");
        }

        let cover_html = if let Some(cover) = &self.cover {
            let x = cover.focal_point.x.get();
            let y = cover.focal_point.y.get();
            format!(
                r#"<tr><td style="padding:12px 24px 0 24px;"><img src="cid:{COVER_CID}" alt="{}" width="472" style="display:block;width:100%;max-width:472px;max-height:300px;object-fit:cover;object-position:{x}% {y}%;border-radius:2px;border:0;" /></td></tr>"#,
                HtmlEscaped(title)
            )
        } else {
            String::from(
                r#"<tr><td align="center" style="padding:20px 32px 0 32px;"><div style="width:40px;height:1px;background-color:#d6cfc4;"></div></td></tr>"#,
            )
        };

        let escaped_message = self
            .message
            .as_str()
            .split('\n')
            .map(|line| HtmlEscaped(line).to_string())
            .collect::<Vec<_>>()
            .join("<br />");

        let access_code_html = if let Some(code) = &self.access_code {
            format!(
                r#"<tr><td align="center" style="padding:0 32px 24px 32px;"><table role="presentation" cellpadding="0" cellspacing="0" style="background-color:#f5f2ec;border:1px solid #e6e0d6;border-radius:4px;"><tr><td align="center" style="padding:12px 20px;"><div style="font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;font-size:11px;letter-spacing:0.12em;text-transform:uppercase;color:#78716c;margin-bottom:4px;">{}</div><div style="font-family:ui-monospace,SFMono-Regular,Menlo,Monaco,Consolas,monospace;font-size:18px;letter-spacing:0.22em;color:#1c1917;font-weight:600;">{}</div></td></tr></table></td></tr>"#,
                HtmlEscaped(copy.access_code_label()),
                HtmlEscaped(code.as_str())
            )
        } else {
            String::new()
        };

        let expiry_html = if let Some(expires_at) = self.expires_at {
            let line = copy.format_expiry(expires_at);
            format!(
                r#"<tr><td align="center" style="padding:0 32px 28px 32px;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;font-size:12px;color:#78716c;">{}</td></tr>"#,
                HtmlEscaped(&line)
            )
        } else {
            String::from(r#"<tr><td style="height:12px;"></td></tr>"#)
        };

        format!(
            r#"<!DOCTYPE html>
<html lang="{lang}">
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head>
<body style="margin:0;padding:0;background-color:#faf8f5;color:#1c1917;">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="background-color:#faf8f5;padding:36px 16px;">
<tr><td align="center">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="max-width:520px;background-color:#ffffff;border:1px solid #e6e0d6;border-radius:4px;">
{signature_html}
{cover_html}
<tr><td align="center" style="padding:28px 32px 8px 32px;">
<div style="font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;font-size:11px;letter-spacing:0.14em;text-transform:uppercase;color:#78716c;margin-bottom:8px;">{eyebrow}</div>
<h1 style="margin:0;font-family:Georgia,'Cormorant Garamond','Times New Roman',serif;font-size:28px;line-height:1.25;font-weight:400;color:#1c1917;">{title}</h1>
</td></tr>
<tr><td align="center" style="padding:12px 36px 28px 36px;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;font-size:15px;line-height:1.65;color:#44403c;">{message}</td></tr>
<tr><td align="center" style="padding:0 32px 28px 32px;">
<a href="{share_url}" style="display:inline-block;background-color:#1c1917;color:#faf8f5;text-decoration:none;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;font-size:13px;font-weight:500;letter-spacing:0.06em;padding:13px 28px;border-radius:3px;">{cta}</a>
</td></tr>
{access_code_html}
{expiry_html}
</table>
</td></tr>
</table>
</body>
</html>"#,
            lang = match self.language {
                Language::En => "en",
                Language::Fr => "fr",
            },
            eyebrow = HtmlEscaped(copy.eyebrow()),
            title = HtmlEscaped(title),
            message = escaped_message,
            share_url = HtmlEscaped(&self.share_url),
            cta = HtmlEscaped(copy.cta()),
        )
    }

    /// Assembles the RFC 5322 / MIME `multipart/alternative` (and `multipart/related` when inline
    /// CID images are present) message ready for SMTP delivery.
    pub fn into_message(self) -> Result<Message, MailError> {
        let copy = EmailCopy {
            stage: self.stage,
            language: self.language,
        };
        let inline_logo = self
            .logo_png
            .as_ref()
            .filter(|_| self.signature.shows_logo())
            .cloned();
        let plain = self.render_plain(copy);
        let html = self.render_html(copy, inline_logo.is_some());

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

    pub const fn from_email(&self) -> &Email {
        &self.from_email
    }

    pub fn from_name(&self) -> Option<&str> {
        self.from_name.as_deref()
    }

    pub async fn send(&self, message: Message) -> Result<(), MailError> {
        self.transport.send(message).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use pp_core::{
        email::{Email, EmailMessage, EmailSubject},
        types::{access_code::GalleryAccessCode, timestamp::Timestamp},
    };
    use pp_database::{
        gallery::{FocalCoordinate, FocalPoint},
        jobs::InvitationStage,
        settings::{CoverSignature, Language},
    };
    use serde_json::from_str;

    use super::{CoverPhoto, WarmPaperEmail};

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

    fn extract_html_part(decoded: &str) -> &str {
        let (_, after_header) = decoded
            .split_once("Content-Type: text/html")
            .expect("HTML MIME part present");

        after_header.split("\r\n--").next().unwrap_or(after_header)
    }

    #[test]
    fn renders_escaped_warm_paper_multipart_with_inline_logo_cover_password_and_expiry() {
        let email = WarmPaperEmail {
            from_email: "noreply@example.test".parse::<Email>().unwrap(),
            from_name: Some("Studio\r\nBcc: attacker@evil.test".to_owned()),
            reply_to: Some("photographer@example.test".parse::<Email>().unwrap()),
            to_email: "client@example.test".parse::<Email>().unwrap(),
            to_name: Some("Alice\r\nCc: leak@evil.test".to_owned()),
            subject: "Your gallery is ready".parse::<EmailSubject>().unwrap(),
            studio_name: "Studio <script>alert(1)</script>".to_owned(),
            signature: CoverSignature::Both,
            logo_png: Some(vec![0x89, b'P', b'N', b'G']),
            cover: Some(CoverPhoto {
                jpeg: vec![0xFF, 0xD8, 0xFF, 0xD9],
                focal_point: FocalPoint {
                    x: FocalCoordinate::try_from(25).unwrap(),
                    y: FocalCoordinate::try_from(75).unwrap(),
                },
            }),
            gallery_title: "Clara & Julien <img src=x onerror=alert(1)>".to_owned(),
            stage: InvitationStage::Proofing,
            language: Language::En,
            message: "Hello!\n<script>evil()</script>"
                .parse::<EmailMessage>()
                .unwrap(),
            share_url: "https://photos.example.test/g/tok_123".to_owned(),
            access_code: Some("042819".parse::<GalleryAccessCode>().unwrap()),
            expires_at: Some(from_str::<Timestamp>("\"2026-12-01T00:00:00Z\"").unwrap()),
        };

        let raw = String::from_utf8(email.into_message().unwrap().formatted()).unwrap();
        let decoded = decode_quoted_printable(&raw);
        let html_part = extract_html_part(&decoded);

        assert!(!raw.contains("\r\nBcc:"));
        assert!(!raw.contains("\r\nCc:"));
        assert!(raw.contains("Reply-To: photographer@example.test"));
        assert!(!html_part.contains("<script>"));
        assert!(!html_part.contains("<img src=x"));
        assert!(html_part.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(html_part.contains("cid:studio-logo@photoproofing"));
        assert!(raw.contains("Content-ID: <studio-logo@photoproofing>"));
        assert!(html_part.contains("cid:gallery-cover@photoproofing"));
        assert!(raw.contains("Content-ID: <gallery-cover@photoproofing>"));
        assert!(html_part.contains("object-position:25% 75%"));
        assert!(decoded.contains("https://photos.example.test/g/tok_123"));
        assert!(decoded.contains("042819"));
        assert!(decoded.contains("December 1, 2026"));
        assert!(decoded.contains("View gallery"));
    }

    #[test]
    fn renders_french_delivery_copy_and_omits_optional_sections_when_absent() {
        let email = WarmPaperEmail {
            from_email: "noreply@example.test".parse::<Email>().unwrap(),
            from_name: None,
            reply_to: None,
            to_email: "client@example.test".parse::<Email>().unwrap(),
            to_name: None,
            subject: "Vos photos retouchées".parse::<EmailSubject>().unwrap(),
            studio_name: "Atelier".to_owned(),
            signature: CoverSignature::None,
            logo_png: Some(vec![1, 2, 3]),
            cover: None,
            gallery_title: "Mariage".to_owned(),
            stage: InvitationStage::Finals,
            language: Language::Fr,
            message: "Voici vos photos.".parse::<EmailMessage>().unwrap(),
            share_url: "https://photos.example.test/g/tok_fr".to_owned(),
            access_code: None,
            expires_at: None,
        };

        let raw = String::from_utf8(email.into_message().unwrap().formatted()).unwrap();
        let decoded = decode_quoted_printable(&raw);

        assert!(!decoded.contains("cid:studio-logo@photoproofing"));
        assert!(!decoded.contains("cid:gallery-cover@photoproofing"));
        assert!(!decoded.contains("Code d’accès"));
        assert!(!decoded.contains("Disponible jusqu’au"));
        assert!(decoded.contains("Photos retouchées prêtes"));
        assert!(decoded.contains("Voir les photos retouchées"));
    }
}
