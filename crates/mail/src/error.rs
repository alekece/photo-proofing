//! Typed error boundary for MIME email composition and SMTP delivery.

use askama::Error as AskamaError;
use lettre::{
    address::AddressError, error::Error as LettreError, message::header::ContentTypeErr,
    transport::smtp::Error as SmtpError,
};
use thiserror::Error;

/// Failure while constructing or delivering an outgoing email.
#[derive(Debug, Error)]
pub enum MailError {
    #[error("invalid email address")]
    Address(#[from] AddressError),
    #[error("invalid MIME content type")]
    ContentType(#[from] ContentTypeErr),
    #[error("failed to render email template")]
    Template(#[from] AskamaError),
    #[error("failed to assemble MIME message")]
    Message(#[from] LettreError),
    #[error("SMTP delivery failed")]
    Smtp(#[from] SmtpError),
}
