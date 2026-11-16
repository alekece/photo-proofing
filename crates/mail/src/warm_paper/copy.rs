//! Localized EN/FR copy and metadata formatting for Warm Paper emails.

use pp_core::types::studio::{InvitationStage, Language};
use time::Date;

use super::EmailKind;

/// Localized copy and formatting rules for one `(EmailKind, Language)` pair.
#[derive(Clone, Copy)]
pub(super) struct EmailCopy {
    kind: EmailKind,
    language: Language,
}

impl EmailCopy {
    pub(super) const fn new(kind: EmailKind, language: Language) -> Self {
        Self { kind, language }
    }

    pub(super) const fn lang_code(self) -> &'static str {
        match self.language {
            Language::En => "en",
            Language::Fr => "fr",
        }
    }

    pub(super) const fn eyebrow(self) -> &'static str {
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

    pub(super) const fn cta(self) -> &'static str {
        match (self.kind, self.language) {
            (EmailKind::Invitation(InvitationStage::Proofing), Language::En) => "View gallery",
            (EmailKind::Invitation(InvitationStage::Proofing), Language::Fr) => "Voir la galerie",
            (EmailKind::Invitation(InvitationStage::Finals), Language::En) => "View final photos",
            (EmailKind::Invitation(InvitationStage::Finals), Language::Fr) => {
                "Voir les photos retouchées"
            }
        }
    }

    pub(super) const fn untitled_gallery(self) -> &'static str {
        match self.language {
            Language::En => "Untitled gallery",
            Language::Fr => "Galerie sans titre",
        }
    }

    pub(super) const fn access_code_label(self) -> &'static str {
        match self.language {
            Language::En => "Access code",
            Language::Fr => "Code d’accès",
        }
    }

    pub(super) fn format_meta_line(self, shoot_date: Option<Date>, photo_count: usize) -> String {
        let count_text = self.format_photo_count(photo_count);

        match shoot_date {
            Some(date) => format!("{} · {count_text}", self.format_long_date(date)),
            None => count_text,
        }
    }

    pub(super) fn format_expiry(self, date: Date) -> String {
        let formatted = self.format_long_date(date);

        match self.language {
            Language::En => format!("Available until {formatted}"),
            Language::Fr => format!("Disponible jusqu’au {formatted}"),
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
}
