//! Workspace contacts and their per-gallery private link associations.
//!
//! Every query is scoped by [`WorkspaceId`], so a contact or gallery of another workspace is
//! indistinguishable from a missing one. Contact emails reuse [`Email`] so addresses are
//! trimmed and lowercased before storage, and `(workspace_id, email)` uniqueness surfaces as
//! [`ContactError::DuplicateEmail`]. Linking a contact to a gallery stores a dedicated
//! [`ShareToken`] on the association without exposing that token in [`Contact`].

use std::str::FromStr;

use derive_more::Display;
use pp_core::{
    email::Email,
    types::{generate::Generate, timestamp::Timestamp},
};
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use sqlx::{AssertSqlSafe, FromRow, PgPool, Postgres, Transaction, Type};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    Repository,
    auth::WorkspaceId,
    error::DatabaseError,
    gallery::{GalleryId, ShareToken},
};

pub const MAX_NAME_CHARS: usize = 120;
pub const MAX_PHONE_CHARS: usize = 40;
pub const MAX_NOTES_CHARS: usize = 2000;

const WORKSPACE_EMAIL_UNIQUE: &str = "contacts_workspace_email_unique";
const GALLERY_CONTACT_TOKEN_UNIQUE: &str = "gallery_contacts_share_token_unique";

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("contact name must contain between 1 and 120 characters")]
pub struct ContactNameError;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("contact phone must contain between 1 and 40 characters")]
pub struct ContactPhoneError;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("contact notes must contain at most 2000 characters")]
pub struct ContactNotesError;

#[derive(Debug, Error)]
pub enum ContactError {
    #[error("a contact in this workspace already uses this email")]
    DuplicateEmail,
    #[error("database error")]
    Database(#[from] DatabaseError),
}

impl ContactError {
    fn classify(error: sqlx::Error) -> Self {
        let is_duplicate = error
            .as_database_error()
            .is_some_and(|db| db.constraint() == Some(WORKSPACE_EMAIL_UNIQUE));

        if is_duplicate {
            return Self::DuplicateEmail;
        }

        Self::Database(DatabaseError::from(error))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct ContactId(pub Uuid);

impl Generate for ContactId {
    fn generate() -> Self {
        Self(Uuid::new_v4())
    }
}

/// A contact's display name, trimmed to between 1 and [`MAX_NAME_CHARS`] characters.
///
/// ```
/// use pp_database::contact::ContactName;
///
/// let name: ContactName = "  Camille & Hugo  ".parse().unwrap();
/// assert_eq!(name.as_str(), "Camille & Hugo");
/// assert!("   ".parse::<ContactName>().is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize, Type)]
#[serde(try_from = "String", into = "String")]
#[sqlx(transparent)]
pub struct ContactName(String);

impl ContactName {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Derives a fallback display name from `email`'s local part (before `@`), bounded to
    /// [`MAX_NAME_CHARS`].
    pub fn from_email_local_part(email: &Email) -> Self {
        let local = email
            .as_str()
            .split_once('@')
            .map_or(email.as_str(), |(local, _)| local)
            .trim();
        let bounded: String = local
            .chars()
            .filter(|ch| *ch != '\0')
            .take(MAX_NAME_CHARS)
            .collect();

        if bounded.is_empty() {
            return Self("Client".to_owned());
        }

        Self(bounded)
    }
}

impl FromStr for ContactName {
    type Err = ContactNameError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let trimmed = value.trim();

        if !(1..=MAX_NAME_CHARS).contains(&trimmed.chars().count()) || trimmed.contains('\0') {
            return Err(ContactNameError);
        }

        Ok(Self(trimmed.to_owned()))
    }
}

impl TryFrom<String> for ContactName {
    type Error = ContactNameError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<ContactName> for String {
    fn from(name: ContactName) -> Self {
        name.0
    }
}

/// An optional phone number, trimmed to between 1 and [`MAX_PHONE_CHARS`] characters.
///
/// ```
/// use pp_database::contact::ContactPhone;
///
/// let phone: ContactPhone = " +33 6 12 34 56 78 ".parse().unwrap();
/// assert_eq!(phone.as_str(), "+33 6 12 34 56 78");
/// assert!("".parse::<ContactPhone>().is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize, Type)]
#[serde(try_from = "String", into = "String")]
#[sqlx(transparent)]
pub struct ContactPhone(String);

impl ContactPhone {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for ContactPhone {
    type Err = ContactPhoneError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let trimmed = value.trim();

        if !(1..=MAX_PHONE_CHARS).contains(&trimmed.chars().count()) || trimmed.contains('\0') {
            return Err(ContactPhoneError);
        }

        Ok(Self(trimmed.to_owned()))
    }
}

impl TryFrom<String> for ContactPhone {
    type Error = ContactPhoneError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<ContactPhone> for String {
    fn from(phone: ContactPhone) -> Self {
        phone.0
    }
}

/// Free-form studio notes on a contact, trimmed to at most [`MAX_NOTES_CHARS`] characters.
///
/// ```
/// use pp_database::contact::ContactNotes;
///
/// let notes: ContactNotes = "  Prefers matte prints  ".parse().unwrap();
/// assert_eq!(notes.as_str(), "Prefers matte prints");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize, Type)]
#[serde(try_from = "String", into = "String")]
#[sqlx(transparent)]
pub struct ContactNotes(String);

impl ContactNotes {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for ContactNotes {
    type Err = ContactNotesError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let trimmed = value.trim();

        if trimmed.chars().count() > MAX_NOTES_CHARS || trimmed.contains('\0') {
            return Err(ContactNotesError);
        }

        Ok(Self(trimmed.to_owned()))
    }
}

impl TryFrom<String> for ContactNotes {
    type Error = ContactNotesError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<ContactNotes> for String {
    fn from(notes: ContactNotes) -> Self {
        notes.0
    }
}

/// A gallery linked to a [`Contact`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkedGallery {
    pub id: GalleryId,
    pub title: Option<String>,
}

/// A workspace contact with the galleries linked to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, FromRow)]
pub struct Contact {
    pub id: ContactId,
    pub name: ContactName,
    pub email: Email,
    pub phone: Option<ContactPhone>,
    pub notes: Option<ContactNotes>,
    #[sqlx(json)]
    pub galleries: Vec<LinkedGallery>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// A contact linked to a gallery together with its per-contact private link token.
///
/// Intentionally omits `Serialize` so route handlers return [`Self::contact`] and never send
/// raw per-contact tokens over photographer JSON endpoints. [`ShareToken`] redacts the secret
/// in [`Debug`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GalleryContactLink {
    pub contact: Contact,
    pub share_token: ShareToken,
}

/// Fields supplied when creating a workspace contact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewContact {
    pub name: ContactName,
    pub email: Email,
    #[serde(default)]
    pub phone: Option<ContactPhone>,
    #[serde(default)]
    pub notes: Option<ContactNotes>,
}

struct Present;

impl Present {
    fn deserialize<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        Option::<T>::deserialize(deserializer)?
            .map(Some)
            .ok_or_else(|| D::Error::custom("null is not allowed"))
    }
}

/// Partial update for a [`Contact`]: omitted fields keep their stored value, `null` clears
/// optional fields (`phone`, `notes`), and `null` on required fields (`name`, `email`) fails
/// deserialization.
///
/// ```
/// use pp_database::contact::ContactChanges;
///
/// let empty: ContactChanges = serde_json::from_str("{}").unwrap();
/// assert!(empty.is_empty());
/// assert!(serde_json::from_str::<ContactChanges>(r#"{"name": null}"#).is_err());
/// ```
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContactChanges {
    #[serde(
        default,
        deserialize_with = "Present::deserialize",
        skip_serializing_if = "Option::is_none"
    )]
    pub name: Option<ContactName>,
    #[serde(
        default,
        deserialize_with = "Present::deserialize",
        skip_serializing_if = "Option::is_none"
    )]
    pub email: Option<Email>,
    #[serde(
        default,
        with = "serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub phone: Option<Option<ContactPhone>>,
    #[serde(
        default,
        with = "serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub notes: Option<Option<ContactNotes>>,
}

impl ContactChanges {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Data access for workspace contacts and gallery-contact links.
#[derive(Clone, Copy)]
pub struct ContactRepository<'db> {
    pool: &'db PgPool,
}

impl<'db> Repository<'db> for ContactRepository<'db> {
    type Options = ();

    fn open(pool: &'db PgPool, (): Self::Options) -> Self {
        Self { pool }
    }
}

impl ContactRepository<'_> {
    const SELECT_CONTACT: &'static str = "SELECT c.id,
                c.name,
                c.email,
                c.phone,
                c.notes,
                COALESCE(
                    (SELECT jsonb_agg(
                                jsonb_build_object('id', g.id, 'title', g.title)
                                ORDER BY lower(g.title) NULLS LAST, g.id
                            )
                     FROM gallery_contacts AS gc
                     JOIN galleries AS g ON g.id = gc.gallery_id
                     WHERE gc.contact_id = c.id AND g.workspace_id = c.workspace_id),
                    '[]'::jsonb
                ) AS galleries,
                c.created_at,
                c.updated_at
         FROM contacts AS c";

    fn is_token_collision(error: &sqlx::Error) -> bool {
        error
            .as_database_error()
            .is_some_and(|db| db.constraint() == Some(GALLERY_CONTACT_TOKEN_UNIQUE))
    }

    /// Lists all contacts in `workspace_id`, ordered alphabetically by name (case-insensitive)
    /// and id, each with its linked workspace galleries.
    pub async fn list(&self, workspace_id: WorkspaceId) -> Result<Vec<Contact>, DatabaseError> {
        Ok(sqlx::query_as(AssertSqlSafe(format!(
            "{} WHERE c.workspace_id = $1 ORDER BY lower(c.name), c.id",
            Self::SELECT_CONTACT
        )))
        .bind(workspace_id)
        .fetch_all(self.pool)
        .await?)
    }

    /// Fetches one contact in `workspace_id`, or `None` if missing or owned by another workspace.
    pub async fn get(
        &self,
        workspace_id: WorkspaceId,
        id: ContactId,
    ) -> Result<Option<Contact>, DatabaseError> {
        Ok(sqlx::query_as(AssertSqlSafe(format!(
            "{} WHERE c.workspace_id = $1 AND c.id = $2",
            Self::SELECT_CONTACT
        )))
        .bind(workspace_id)
        .bind(id)
        .fetch_optional(self.pool)
        .await?)
    }

    /// Creates a new workspace contact. Returns [`ContactError::DuplicateEmail`] when another
    /// contact in `workspace_id` already uses the normalized email.
    pub async fn create(
        &self,
        workspace_id: WorkspaceId,
        input: &NewContact,
    ) -> Result<Contact, ContactError> {
        let id = ContactId::generate();
        let notes = input
            .notes
            .as_ref()
            .filter(|notes| !notes.as_str().is_empty());

        sqlx::query_as(
            "INSERT INTO contacts (id, workspace_id, name, email, phone, notes)
             VALUES ($1, $2, $3, $4, $5, $6)
             RETURNING id, name, email, phone, notes, '[]'::jsonb AS galleries, created_at, updated_at",
        )
        .bind(id)
        .bind(workspace_id)
        .bind(&input.name)
        .bind(&input.email)
        .bind(&input.phone)
        .bind(notes)
        .fetch_one(self.pool)
        .await
        .map_err(ContactError::classify)
    }

    /// Updates `id` in `workspace_id`. An empty changeset reads without bumping `updated_at`.
    pub async fn update(
        &self,
        workspace_id: WorkspaceId,
        id: ContactId,
        changes: &ContactChanges,
    ) -> Result<Option<Contact>, ContactError> {
        if changes.is_empty() {
            return Ok(self.get(workspace_id, id).await?);
        }

        let notes = changes
            .notes
            .as_ref()
            .map(|value| value.as_ref().filter(|notes| !notes.as_str().is_empty()));

        let updated_id: Option<ContactId> = sqlx::query_scalar(
            "UPDATE contacts SET
                 name = COALESCE($3, name),
                 email = COALESCE($4, email),
                 phone = CASE WHEN $5 THEN $6 ELSE phone END,
                 notes = CASE WHEN $7 THEN $8 ELSE notes END,
                 updated_at = now()
             WHERE workspace_id = $1 AND id = $2
             RETURNING id",
        )
        .bind(workspace_id)
        .bind(id)
        .bind(&changes.name)
        .bind(&changes.email)
        .bind(changes.phone.is_some())
        .bind(changes.phone.as_ref().and_then(Option::as_ref))
        .bind(notes.is_some())
        .bind(notes.flatten())
        .fetch_optional(self.pool)
        .await
        .map_err(ContactError::classify)?;

        let Some(updated_id) = updated_id else {
            return Ok(None);
        };

        Ok(self.get(workspace_id, updated_id).await?)
    }

    /// Deletes a contact in `workspace_id`, cascading its gallery links.
    pub async fn delete(
        &self,
        workspace_id: WorkspaceId,
        id: ContactId,
    ) -> Result<bool, DatabaseError> {
        let result = sqlx::query("DELETE FROM contacts WHERE workspace_id = $1 AND id = $2")
            .bind(workspace_id)
            .bind(id)
            .execute(self.pool)
            .await?;

        Ok(result.rows_affected() == 1)
    }

    /// Lists contacts linked to `gallery_id` in `workspace_id`, or `None` if the workspace has no
    /// such gallery.
    pub async fn list_for_gallery(
        &self,
        workspace_id: WorkspaceId,
        gallery_id: GalleryId,
    ) -> Result<Option<Vec<Contact>>, DatabaseError> {
        let gallery_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM galleries WHERE workspace_id = $1 AND id = $2)",
        )
        .bind(workspace_id)
        .bind(gallery_id)
        .fetch_one(self.pool)
        .await?;

        if !gallery_exists {
            return Ok(None);
        }

        let contacts = sqlx::query_as(AssertSqlSafe(format!(
            "{} JOIN gallery_contacts AS link
                    ON link.contact_id = c.id AND link.gallery_id = $2
             WHERE c.workspace_id = $1
             ORDER BY lower(c.name), c.id",
            Self::SELECT_CONTACT
        )))
        .bind(workspace_id)
        .bind(gallery_id)
        .fetch_all(self.pool)
        .await?;

        Ok(Some(contacts))
    }

    /// Fetches a contact and its dedicated [`ShareToken`] for `gallery_id` in `workspace_id`,
    /// or `None` when the contact, gallery, or link does not exist in that workspace.
    pub async fn find_gallery_link(
        &self,
        workspace_id: WorkspaceId,
        gallery_id: GalleryId,
        contact_id: ContactId,
    ) -> Result<Option<GalleryContactLink>, DatabaseError> {
        let share_token: Option<ShareToken> = sqlx::query_scalar(
            "SELECT gc.share_token
             FROM gallery_contacts AS gc
             JOIN galleries AS g ON g.id = gc.gallery_id
             JOIN contacts AS c ON c.id = gc.contact_id
             WHERE g.workspace_id = $1 AND c.workspace_id = $1
               AND gc.gallery_id = $2 AND gc.contact_id = $3",
        )
        .bind(workspace_id)
        .bind(gallery_id)
        .bind(contact_id)
        .fetch_optional(self.pool)
        .await?;

        let Some(share_token) = share_token else {
            return Ok(None);
        };

        let Some(contact) = self.get(workspace_id, contact_id).await? else {
            return Ok(None);
        };

        Ok(Some(GalleryContactLink {
            contact,
            share_token,
        }))
    }

    /// Ensures `(workspace_id, email)` exists in `contacts` without overwriting an existing
    /// contact's name, and idempotently links that contact to `gallery_id` inside `tx`.
    pub(crate) async fn upsert_and_link_in(
        tx: &mut Transaction<'_, Postgres>,
        workspace_id: WorkspaceId,
        gallery_id: GalleryId,
        email: &Email,
        fallback_name: &ContactName,
        mut next_token: impl FnMut() -> ShareToken,
    ) -> Result<ContactId, DatabaseError> {
        sqlx::query(
            "INSERT INTO contacts (id, workspace_id, name, email)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (workspace_id, email) DO NOTHING",
        )
        .bind(ContactId::generate())
        .bind(workspace_id)
        .bind(fallback_name)
        .bind(email)
        .execute(&mut **tx)
        .await?;

        let contact_id: ContactId =
            sqlx::query_scalar("SELECT id FROM contacts WHERE workspace_id = $1 AND email = $2")
                .bind(workspace_id)
                .bind(email)
                .fetch_one(&mut **tx)
                .await?;

        let already_linked: bool = sqlx::query_scalar(
            "SELECT EXISTS (
                 SELECT 1 FROM gallery_contacts WHERE gallery_id = $1 AND contact_id = $2
             )",
        )
        .bind(gallery_id)
        .bind(contact_id)
        .fetch_one(&mut **tx)
        .await?;

        if already_linked {
            return Ok(contact_id);
        }

        loop {
            sqlx::query("SAVEPOINT contact_share_token")
                .execute(&mut **tx)
                .await?;

            let candidate = next_token();
            let insert_result = sqlx::query(
                "INSERT INTO gallery_contacts (gallery_id, contact_id, share_token)
                 SELECT $1, $2, $3
                 WHERE NOT EXISTS (SELECT 1 FROM galleries WHERE share_token = $3)
                 ON CONFLICT (gallery_id, contact_id) DO NOTHING",
            )
            .bind(gallery_id)
            .bind(contact_id)
            .bind(&candidate)
            .execute(&mut **tx)
            .await;

            match insert_result {
                Ok(outcome) => {
                    sqlx::query("RELEASE SAVEPOINT contact_share_token")
                        .execute(&mut **tx)
                        .await?;

                    if outcome.rows_affected() == 1 {
                        return Ok(contact_id);
                    }

                    let linked: bool = sqlx::query_scalar(
                        "SELECT EXISTS (
                             SELECT 1 FROM gallery_contacts
                             WHERE gallery_id = $1 AND contact_id = $2
                         )",
                    )
                    .bind(gallery_id)
                    .bind(contact_id)
                    .fetch_one(&mut **tx)
                    .await?;

                    if linked {
                        return Ok(contact_id);
                    }
                }
                Err(error) if Self::is_token_collision(&error) => {
                    sqlx::query("ROLLBACK TO SAVEPOINT contact_share_token")
                        .execute(&mut **tx)
                        .await?;
                }
                Err(error) => return Err(error.into()),
            }
        }
    }

    /// Idempotently links `contact_id` to `gallery_id` within `workspace_id`, generating a
    /// dedicated [`ShareToken`] on first link. Returns `None` if either entity is missing or
    /// belongs to another workspace.
    pub async fn link_gallery(
        &self,
        workspace_id: WorkspaceId,
        gallery_id: GalleryId,
        contact_id: ContactId,
    ) -> Result<Option<GalleryContactLink>, DatabaseError> {
        self.link_gallery_with(workspace_id, gallery_id, contact_id, ShareToken::generate)
            .await
    }

    pub(crate) async fn link_gallery_with(
        &self,
        workspace_id: WorkspaceId,
        gallery_id: GalleryId,
        contact_id: ContactId,
        mut next_token: impl FnMut() -> ShareToken,
    ) -> Result<Option<GalleryContactLink>, DatabaseError> {
        loop {
            let candidate = next_token();
            let insert_result = sqlx::query(
                "INSERT INTO gallery_contacts (gallery_id, contact_id, share_token)
                 SELECT g.id, c.id, $4
                 FROM galleries AS g
                 JOIN contacts AS c ON c.workspace_id = g.workspace_id
                 WHERE g.workspace_id = $1 AND g.id = $2 AND c.id = $3
                   AND NOT EXISTS (SELECT 1 FROM galleries WHERE share_token = $4)
                 ON CONFLICT (gallery_id, contact_id) DO NOTHING",
            )
            .bind(workspace_id)
            .bind(gallery_id)
            .bind(contact_id)
            .bind(&candidate)
            .execute(self.pool)
            .await;

            match insert_result {
                Ok(_) => {}
                Err(error) if Self::is_token_collision(&error) => continue,
                Err(error) => return Err(error.into()),
            }

            let existing_token: Option<ShareToken> = sqlx::query_scalar(
                "SELECT gc.share_token
                 FROM gallery_contacts AS gc
                 JOIN galleries AS g ON g.id = gc.gallery_id
                 JOIN contacts AS c ON c.id = gc.contact_id
                 WHERE g.workspace_id = $1 AND c.workspace_id = $1
                   AND gc.gallery_id = $2 AND gc.contact_id = $3",
            )
            .bind(workspace_id)
            .bind(gallery_id)
            .bind(contact_id)
            .fetch_optional(self.pool)
            .await?;

            if let Some(share_token) = existing_token {
                if let Some(contact) = self.get(workspace_id, contact_id).await? {
                    return Ok(Some(GalleryContactLink {
                        contact,
                        share_token,
                    }));
                }

                return Ok(None);
            }

            let pair_exists: bool = sqlx::query_scalar(
                "SELECT EXISTS (
                     SELECT 1
                     FROM galleries AS g
                     JOIN contacts AS c ON c.workspace_id = g.workspace_id
                     WHERE g.workspace_id = $1 AND g.id = $2 AND c.id = $3
                 )",
            )
            .bind(workspace_id)
            .bind(gallery_id)
            .bind(contact_id)
            .fetch_one(self.pool)
            .await?;

            if !pair_exists {
                return Ok(None);
            }
        }
    }

    /// Removes the link between `gallery_id` and `contact_id` in `workspace_id` without deleting
    /// the contact itself.
    pub async fn unlink_gallery(
        &self,
        workspace_id: WorkspaceId,
        gallery_id: GalleryId,
        contact_id: ContactId,
    ) -> Result<bool, DatabaseError> {
        let result = sqlx::query(
            "DELETE FROM gallery_contacts AS gc
             USING galleries AS g, contacts AS c
             WHERE gc.gallery_id = g.id AND gc.contact_id = c.id
               AND g.workspace_id = $1 AND c.workspace_id = $1
               AND gc.gallery_id = $2 AND gc.contact_id = $3",
        )
        .bind(workspace_id)
        .bind(gallery_id)
        .bind(contact_id)
        .execute(self.pool)
        .await?;

        Ok(result.rows_affected() == 1)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sqlx::AssertSqlSafe;

    use super::*;
    use crate::{
        Database,
        gallery::{GalleryChanges, GalleryRepository},
        jobs::JobOptions,
        photo::PhotoLocation,
        workspace_deletion::WorkspaceDeletion,
    };

    async fn workspace(pool: &PgPool) -> WorkspaceId {
        let id = WorkspaceId(Uuid::new_v4());

        sqlx::query("INSERT INTO workspaces(id) VALUES ($1)")
            .bind(id)
            .execute(pool)
            .await
            .unwrap();

        id
    }

    fn new_contact(name: &str, email: &str) -> NewContact {
        NewContact {
            name: name.parse().unwrap(),
            email: email.parse().unwrap(),
            phone: None,
            notes: None,
        }
    }

    #[test]
    fn contact_name_trims_bounds_length_and_rejects_blank_or_nul() {
        let parsed: ContactName = "  Camille & Hugo  ".parse().unwrap();

        assert_eq!(parsed.as_str(), "Camille & Hugo");
        assert_eq!(
            "é".repeat(MAX_NAME_CHARS)
                .parse::<ContactName>()
                .unwrap()
                .as_str()
                .chars()
                .count(),
            MAX_NAME_CHARS
        );

        for invalid in ["", "   ", &"é".repeat(MAX_NAME_CHARS + 1), "Camille\0Hugo"] {
            assert!(invalid.parse::<ContactName>().is_err(), "{invalid:?}");
        }
    }

    #[test]
    fn contact_phone_trims_bounds_length_and_rejects_blank_or_nul() {
        let parsed: ContactPhone = "  +33 6 12 34 56 78  ".parse().unwrap();

        assert_eq!(parsed.as_str(), "+33 6 12 34 56 78");
        assert_eq!(
            "1".repeat(MAX_PHONE_CHARS)
                .parse::<ContactPhone>()
                .unwrap()
                .as_str()
                .chars()
                .count(),
            MAX_PHONE_CHARS
        );

        for invalid in [
            "",
            "   ",
            &"1".repeat(MAX_PHONE_CHARS + 1),
            "+33\x00600000000",
        ] {
            assert!(invalid.parse::<ContactPhone>().is_err(), "{invalid:?}");
        }
    }

    #[test]
    fn contact_notes_trims_bounds_length_and_rejects_nul() {
        let parsed: ContactNotes = "  Prefers matte prints\nVIP  ".parse().unwrap();

        assert_eq!(parsed.as_str(), "Prefers matte prints\nVIP");
        assert_eq!("   ".parse::<ContactNotes>().unwrap().as_str(), "");
        assert_eq!(
            "é".repeat(MAX_NOTES_CHARS)
                .parse::<ContactNotes>()
                .unwrap()
                .as_str()
                .chars()
                .count(),
            MAX_NOTES_CHARS
        );

        for invalid in [&"é".repeat(MAX_NOTES_CHARS + 1), "note\0here"] {
            assert!(invalid.parse::<ContactNotes>().is_err(), "{invalid:?}");
        }
    }

    #[test]
    fn contact_changes_rejects_null_required_fields_and_unknown_keys() {
        for invalid in [
            json!({"name": null}),
            json!({"email": null}),
            json!({"email": "not-an-email"}),
            json!({"name": "   "}),
            json!({"phone": "   "}),
            json!({"unknown": "field"}),
        ] {
            assert!(
                serde_json::from_value::<ContactChanges>(invalid.clone()).is_err(),
                "{invalid}"
            );
        }

        let empty: ContactChanges = serde_json::from_value(json!({})).unwrap();

        assert!(empty.is_empty());

        let cleared: ContactChanges =
            serde_json::from_value(json!({"phone": null, "notes": null})).unwrap();

        assert_eq!(cleared.phone, Some(None));
        assert_eq!(cleared.notes, Some(None));
        assert!(!cleared.is_empty());
    }

    #[test]
    fn gallery_contact_link_redacts_share_token_in_debug() {
        let token = ShareToken::generate();
        let secret = token.to_owned();
        let link = GalleryContactLink {
            contact: Contact {
                id: ContactId::generate(),
                name: "Camille".parse().unwrap(),
                email: "camille@example.test".parse().unwrap(),
                phone: None,
                notes: None,
                galleries: Vec::new(),
                created_at: serde_json::from_value(json!("2026-11-13T12:00:00Z")).unwrap(),
                updated_at: serde_json::from_value(json!("2026-11-13T12:00:00Z")).unwrap(),
            },
            share_token: token,
        };

        assert!(!format!("{link:?}").contains(secret.as_str()));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn migration_enforces_contact_and_token_constraints(pool: PgPool) {
        let workspace_id = workspace(&pool).await;
        let contact_id = Uuid::new_v4();

        for bad_name in ["", "   ", &"a".repeat(MAX_NAME_CHARS + 1)] {
            let mut tx = pool.begin().await.unwrap();
            let res = sqlx::query(
                "INSERT INTO contacts(id, workspace_id, name, email) VALUES ($1, $2, $3, 'ok@example.test')",
            )
            .bind(Uuid::new_v4())
            .bind(workspace_id)
            .bind(bad_name)
            .execute(&mut *tx)
            .await;

            assert!(res.is_err(), "expected name check failure for {bad_name:?}");
        }

        for bad_email in ["ab", &"a".repeat(255)] {
            let mut tx = pool.begin().await.unwrap();
            let res = sqlx::query(
                "INSERT INTO contacts(id, workspace_id, name, email) VALUES ($1, $2, 'Alice', $3)",
            )
            .bind(Uuid::new_v4())
            .bind(workspace_id)
            .bind(bad_email)
            .execute(&mut *tx)
            .await;

            assert!(res.is_err(), "expected email check failure");
        }

        for bad_phone in ["", &"1".repeat(MAX_PHONE_CHARS + 1)] {
            let mut tx = pool.begin().await.unwrap();
            let res = sqlx::query(
                "INSERT INTO contacts(id, workspace_id, name, email, phone) VALUES ($1, $2, 'Alice', 'p@example.test', $3)",
            )
            .bind(Uuid::new_v4())
            .bind(workspace_id)
            .bind(bad_phone)
            .execute(&mut *tx)
            .await;

            assert!(res.is_err(), "expected phone check failure");
        }

        {
            let mut tx = pool.begin().await.unwrap();
            let res = sqlx::query(
                "INSERT INTO contacts(id, workspace_id, name, email, notes) VALUES ($1, $2, 'Alice', 'n@example.test', $3)",
            )
            .bind(Uuid::new_v4())
            .bind(workspace_id)
            .bind("n".repeat(MAX_NOTES_CHARS + 1))
            .execute(&mut *tx)
            .await;

            assert!(res.is_err(), "expected notes check failure");
        }

        sqlx::query(
            "INSERT INTO contacts(id, workspace_id, name, email) VALUES ($1, $2, 'Alice', 'alice@example.test')",
        )
        .bind(contact_id)
        .bind(workspace_id)
        .execute(&pool)
        .await
        .unwrap();

        let gallery_id = Uuid::new_v4();

        sqlx::query("INSERT INTO galleries(id, workspace_id) VALUES ($1, $2)")
            .bind(gallery_id)
            .bind(workspace_id)
            .execute(&pool)
            .await
            .unwrap();

        {
            let mut tx = pool.begin().await.unwrap();
            let res = sqlx::query(
                "INSERT INTO gallery_contacts(gallery_id, contact_id, share_token) VALUES ($1, $2, 'short')",
            )
            .bind(gallery_id)
            .bind(contact_id)
            .execute(&mut *tx)
            .await;

            assert!(res.is_err(), "expected share_token shape check failure");
        }

        let valid_token = ShareToken::generate();

        sqlx::query(
            "INSERT INTO gallery_contacts(gallery_id, contact_id, share_token) VALUES ($1, $2, $3)",
        )
        .bind(gallery_id)
        .bind(contact_id)
        .bind(&valid_token)
        .execute(&pool)
        .await
        .unwrap();

        // Duplicate gallery_contacts.share_token is rejected.
        let second_gallery_id = Uuid::new_v4();

        sqlx::query("INSERT INTO galleries(id, workspace_id) VALUES ($1, $2)")
            .bind(second_gallery_id)
            .bind(workspace_id)
            .execute(&pool)
            .await
            .unwrap();

        {
            let mut tx = pool.begin().await.unwrap();
            let dup = sqlx::query(
                "INSERT INTO gallery_contacts(gallery_id, contact_id, share_token) VALUES ($1, $2, $3)",
            )
            .bind(second_gallery_id)
            .bind(contact_id)
            .bind(&valid_token)
            .execute(&mut *tx)
            .await;

            assert!(dup.is_err(), "expected duplicate share_token failure");
        }

        // Deleting a gallery cascades its gallery_contacts; deleting the workspace cascades contacts.
        let mut tx = pool.begin().await.unwrap();

        sqlx::query("DELETE FROM galleries WHERE workspace_id = $1")
            .bind(workspace_id)
            .execute(&mut *tx)
            .await
            .unwrap();

        let remaining_links: i64 =
            sqlx::query_scalar("SELECT count(*) FROM gallery_contacts WHERE contact_id = $1")
                .bind(contact_id)
                .fetch_one(&mut *tx)
                .await
                .unwrap();

        assert_eq!(remaining_links, 0);

        sqlx::query("DELETE FROM workspaces WHERE id = $1")
            .bind(workspace_id)
            .execute(&mut *tx)
            .await
            .unwrap();

        let remaining_contacts: i64 =
            sqlx::query_scalar("SELECT count(*) FROM contacts WHERE id = $1")
                .bind(contact_id)
                .fetch_one(&mut *tx)
                .await
                .unwrap();

        assert_eq!(remaining_contacts, 0);
        tx.rollback().await.unwrap();
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn crud_lists_alphabetically_with_linked_galleries_and_isolates_workspaces(pool: PgPool) {
        let database = Database::new(pool.clone());
        let contacts = database.repository::<ContactRepository>();
        let galleries = database.repository::<GalleryRepository>();
        let (mine, other) = (workspace(&pool).await, workspace(&pool).await);

        let zoe = contacts
            .create(
                mine,
                &NewContact {
                    name: "  Zoé Laurent ".parse().unwrap(),
                    email: " Zoe@Example.test ".parse().unwrap(),
                    phone: Some(" +33 6 11 22 33 44 ".parse().unwrap()),
                    notes: Some("  Bride's sister ".parse().unwrap()),
                },
            )
            .await
            .unwrap();
        let camille = contacts
            .create(mine, &new_contact("camille & hugo", "camille@example.test"))
            .await
            .unwrap();

        assert_eq!(zoe.name.as_str(), "Zoé Laurent");
        assert_eq!(zoe.email.as_str(), "zoe@example.test");
        assert_eq!(
            zoe.phone.as_ref().map(ContactPhone::as_str),
            Some("+33 6 11 22 33 44")
        );
        assert_eq!(
            zoe.notes.as_ref().map(ContactNotes::as_str),
            Some("Bride's sister")
        );
        assert!(zoe.galleries.is_empty());

        // Duplicate email within the same workspace is rejected case-insensitively.
        let duplicate = contacts
            .create(mine, &new_contact("Another Zoé", "ZOE@example.test"))
            .await;

        assert!(matches!(duplicate, Err(ContactError::DuplicateEmail)));

        // Another workspace can use the exact same email independently.
        let other_zoe = contacts
            .create(other, &new_contact("Zoé Other", "zoe@example.test"))
            .await
            .unwrap();

        assert_ne!(other_zoe.id, zoe.id);
        assert_eq!(contacts.get(other, zoe.id).await.unwrap(), None);

        // Link Camille to two galleries and check alphabetical ordering on both contacts and galleries.
        let gallery_b = galleries.create(mine).await.unwrap();
        let gallery_a = galleries.create(mine).await.unwrap();

        galleries
            .update(
                mine,
                gallery_b.id,
                &GalleryChanges {
                    title: Some(Some("Wedding Day".to_owned())),
                    ..GalleryChanges::default()
                },
            )
            .await
            .unwrap();
        galleries
            .update(
                mine,
                gallery_a.id,
                &GalleryChanges {
                    title: Some(Some(" engagement ".to_owned())),
                    ..GalleryChanges::default()
                },
            )
            .await
            .unwrap();

        contacts
            .link_gallery(mine, gallery_b.id, camille.id)
            .await
            .unwrap()
            .unwrap();
        contacts
            .link_gallery(mine, gallery_a.id, camille.id)
            .await
            .unwrap()
            .unwrap();

        let listed = contacts.list(mine).await.unwrap();

        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].id, camille.id);
        assert_eq!(listed[1].id, zoe.id);
        assert_eq!(
            listed[0].galleries,
            vec![
                LinkedGallery {
                    id: gallery_a.id,
                    title: Some("engagement".to_owned()),
                },
                LinkedGallery {
                    id: gallery_b.id,
                    title: Some("Wedding Day".to_owned()),
                },
            ]
        );

        // Empty update does not bump updated_at.
        let unchanged = contacts
            .update(mine, zoe.id, &ContactChanges::default())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(unchanged.updated_at, zoe.updated_at);

        // Updating to another contact's email in the same workspace returns DuplicateEmail.
        let email_conflict = contacts
            .update(
                mine,
                zoe.id,
                &serde_json::from_value(json!({"email": "CAMILLE@example.test"})).unwrap(),
            )
            .await;

        assert!(matches!(email_conflict, Err(ContactError::DuplicateEmail)));

        // Partial update keeps omitted fields and clears nulled optional fields.
        let updated = contacts
            .update(
                mine,
                zoe.id,
                &serde_json::from_value(json!({
                    "name": "Zoé L.",
                    "phone": null,
                    "notes": "   "
                }))
                .unwrap(),
            )
            .await
            .unwrap()
            .unwrap();

        assert_eq!(updated.name.as_str(), "Zoé L.");
        assert_eq!(updated.email.as_str(), "zoe@example.test");
        assert_eq!(updated.phone, None);
        assert_eq!(updated.notes, None);
        assert!(updated.updated_at >= zoe.updated_at);

        // Foreign workspace cannot update or delete.
        assert_eq!(
            contacts
                .update(
                    other,
                    zoe.id,
                    &serde_json::from_value(json!({"name": "Stolen"})).unwrap(),
                )
                .await
                .unwrap(),
            None
        );
        assert!(!contacts.delete(other, camille.id).await.unwrap());

        // Deleting a contact cascades gallery_contacts while leaving the galleries intact.
        assert!(contacts.delete(mine, camille.id).await.unwrap());
        assert!(!contacts.delete(mine, camille.id).await.unwrap());
        assert_eq!(contacts.get(mine, camille.id).await.unwrap(), None);
        assert!(galleries.get(mine, gallery_a.id).await.unwrap().is_some());
        assert_eq!(
            contacts
                .list_for_gallery(mine, gallery_a.id)
                .await
                .unwrap()
                .unwrap(),
            Vec::new()
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn linking_is_idempotent_isolated_and_avoids_token_collisions(pool: PgPool) {
        let database = Database::new(pool.clone());
        let contacts = database.repository::<ContactRepository>();
        let galleries = database.repository::<GalleryRepository>();
        let (mine, other) = (workspace(&pool).await, workspace(&pool).await);

        let gallery = galleries.create(mine).await.unwrap();
        let second_gallery = galleries.create(mine).await.unwrap();
        let foreign_gallery = galleries.create(other).await.unwrap();
        let alice = contacts
            .create(mine, &new_contact("Alice", "alice@example.test"))
            .await
            .unwrap();
        let bob = contacts
            .create(mine, &new_contact("Bob", "bob@example.test"))
            .await
            .unwrap();
        let foreign_contact = contacts
            .create(other, &new_contact("Eve", "eve@example.test"))
            .await
            .unwrap();

        // Cross-workspace links and listings return None.
        assert_eq!(
            contacts
                .link_gallery(mine, gallery.id, foreign_contact.id)
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            contacts
                .link_gallery(mine, foreign_gallery.id, alice.id)
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            contacts.list_for_gallery(other, gallery.id).await.unwrap(),
            None
        );

        let shared_gallery = galleries.share(mine, gallery.id).await.unwrap().unwrap();
        let gallery_token = shared_gallery.share_token.unwrap();

        // First candidate collides with galleries.share_token -> retries and picks fresh token.
        let fresh_token = ShareToken::generate();
        let mut candidates = vec![gallery_token.clone(), fresh_token.clone()].into_iter();
        let first_link = contacts
            .link_gallery_with(mine, gallery.id, alice.id, || candidates.next().unwrap())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(first_link.share_token, fresh_token);
        assert_ne!(first_link.share_token, gallery_token);

        // Linking the same (gallery, contact) again is idempotent and keeps the existing token.
        let repeat_link = contacts
            .link_gallery(mine, gallery.id, alice.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(repeat_link.share_token, first_link.share_token);

        // First candidate collides with another gallery_contacts.share_token -> retries and picks next.
        let bob_token = ShareToken::generate();
        let mut bob_candidates =
            vec![first_link.share_token.clone(), bob_token.clone()].into_iter();
        let bob_link = contacts
            .link_gallery_with(mine, gallery.id, bob.id, || bob_candidates.next().unwrap())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(bob_link.share_token, bob_token);

        //GalleryRepository::share_with also avoids an existing gallery_contacts.share_token.
        let second_gallery_token = ShareToken::generate();
        let mut share_candidates =
            vec![bob_link.share_token.clone(), second_gallery_token.clone()].into_iter();
        let shared_second = galleries
            .share_with(mine, second_gallery.id, || share_candidates.next().unwrap())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(shared_second.share_token, Some(second_gallery_token));

        let gallery_contacts = contacts
            .list_for_gallery(mine, gallery.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            gallery_contacts.iter().map(|c| c.id).collect::<Vec<_>>(),
            vec![alice.id, bob.id]
        );

        // Unlinking removes the association without deleting the workspace contact.
        assert!(
            !contacts
                .unlink_gallery(other, gallery.id, alice.id)
                .await
                .unwrap()
        );
        assert!(
            contacts
                .unlink_gallery(mine, gallery.id, alice.id)
                .await
                .unwrap()
        );
        assert!(
            !contacts
                .unlink_gallery(mine, gallery.id, alice.id)
                .await
                .unwrap()
        );
        assert!(contacts.get(mine, alice.id).await.unwrap().is_some());

        // Deleting a gallery cascades its remaining gallery_contacts while leaving Bob intact.
        let deletion = database.repository_with::<WorkspaceDeletion>(JobOptions::default());

        assert!(deletion.delete_gallery(mine, gallery.id).await.unwrap());

        let bob_after = contacts.get(mine, bob.id).await.unwrap().unwrap();

        assert!(bob_after.galleries.is_empty());
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn per_contact_token_grants_shared_access_only_while_gallery_is_shared_and_linked(
        pool: PgPool,
    ) {
        let database = Database::new(pool.clone());
        let contacts = database.repository::<ContactRepository>();
        let galleries = database.repository::<GalleryRepository>();
        let workspace_id = workspace(&pool).await;

        let gallery = galleries.create(workspace_id).await.unwrap();
        let contact = contacts
            .create(
                workspace_id,
                &new_contact("Camille", "camille@example.test"),
            )
            .await
            .unwrap();

        // Linking a contact to a Draft gallery MUST NOT grant client access before sharing!
        let linked = contacts
            .link_gallery(workspace_id, gallery.id, contact.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            galleries.find_shared(&linked.share_token).await.unwrap(),
            None
        );

        // Even if shared_at is NULL with a manual share_token check, only shared galleries resolve.
        let shared = galleries
            .share(workspace_id, gallery.id)
            .await
            .unwrap()
            .unwrap();
        let general_token = shared.share_token.unwrap();

        let general_access = galleries
            .find_shared(&general_token)
            .await
            .unwrap()
            .unwrap();
        let contact_access = galleries
            .find_shared(&linked.share_token)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(general_access.contact_id, None);
        assert_eq!(contact_access.contact_id, Some(contact.id));
        assert_eq!(
            contact_access.location,
            PhotoLocation {
                workspace_id,
                gallery_id: gallery.id,
            }
        );
        assert_eq!(contact_access.gallery, general_access.gallery);
        assert_eq!(
            contact_access.watermark_enabled,
            general_access.watermark_enabled
        );

        // Archiving blocks both tokens; restoring re-enables both.
        galleries.archive(workspace_id, gallery.id).await.unwrap();
        assert_eq!(galleries.find_shared(&general_token).await.unwrap(), None);
        assert_eq!(
            galleries.find_shared(&linked.share_token).await.unwrap(),
            None
        );

        galleries.restore(workspace_id, gallery.id).await.unwrap();
        assert!(
            galleries
                .find_shared(&general_token)
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            galleries
                .find_shared(&linked.share_token)
                .await
                .unwrap()
                .is_some()
        );

        // Expiry blocks both tokens; clearing expiry re-enables both.
        sqlx::query(AssertSqlSafe(
            "UPDATE galleries SET expires_at = now() - interval '1 hour' WHERE id = $1",
        ))
        .bind(gallery.id)
        .execute(&pool)
        .await
        .unwrap();

        assert_eq!(galleries.find_shared(&general_token).await.unwrap(), None);
        assert_eq!(
            galleries.find_shared(&linked.share_token).await.unwrap(),
            None
        );

        sqlx::query(AssertSqlSafe(
            "UPDATE galleries SET expires_at = NULL WHERE id = $1",
        ))
        .bind(gallery.id)
        .execute(&pool)
        .await
        .unwrap();

        // Unlinking immediately revokes ONLY that contact's token while general token stays valid.
        assert!(
            contacts
                .unlink_gallery(workspace_id, gallery.id, contact.id)
                .await
                .unwrap()
        );
        assert_eq!(
            galleries.find_shared(&linked.share_token).await.unwrap(),
            None
        );
        assert!(
            galleries
                .find_shared(&general_token)
                .await
                .unwrap()
                .is_some()
        );

        // Re-linking issues a new token; deleting the contact revokes it while general token stays valid.
        let relinked = contacts
            .link_gallery(workspace_id, gallery.id, contact.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            galleries
                .find_shared(&relinked.share_token)
                .await
                .unwrap()
                .unwrap()
                .contact_id,
            Some(contact.id)
        );
        assert!(contacts.delete(workspace_id, contact.id).await.unwrap());
        assert_eq!(
            galleries.find_shared(&relinked.share_token).await.unwrap(),
            None
        );
        assert!(
            galleries
                .find_shared(&general_token)
                .await
                .unwrap()
                .is_some()
        );
    }
}
