# Deduplicate Rust bounded newtypes, database helpers, and API route plumbing

Issue: [#377](https://github.com/alekece/photo-proofing/issues/377).

## Intent

Eliminate ~300 lines of copy-pasted bounded newtype boilerplate, patch deserialization, `ShareToken` collision-retry loops, two-query `ContactRepository` round-trips, mirrored invitation DTOs, and `SmtpPasswordValueParser` across `pp-core`, `pp-database`, and `pp-api` while keeping all HTTP status codes, JSON wire shapes, and database rules unchanged.

## Goals

- **Const-generic bounded types (`pp-core::types::bounded`):**
  - Introduce `BoundedU8<const MIN: u8, const MAX: u8>` (`i16` storage for `#[sqlx(transparent)]` PostgreSQL `SMALLINT`, `#[serde(try_from = "u8", into = "u8")]`, `BoundedU8::new(u8) -> Result<Self, BoundedU8Error>`, `get(self) -> u8`, `MIN` / `MAX` constants). Define `FocalCoordinate = BoundedU8<0, 100>` (`DEFAULT_X = 50`, `DEFAULT_Y = 35`), `GridCell = BoundedU8<0, 8>`, and `SizeStep = BoundedU8<0, 9>`, removing the unreachable `SettingsError::GridCell` / `SettingsError::SizeStep` variants and dead `ErrorCode::InvalidPosition` / `ErrorCode::InvalidSize` mappings.
  - Introduce `BoundedText<const MIN: usize, const MAX: usize, L = MultiLine>` parameterized by a sealed line-policy marker (`SingleLine` and `MultiLine`, replacing raw boolean flags per `crates/AGENTS.md`): `String` storage + `PhantomData<fn() -> L>`, `#[serde(try_from = "String", into = "String")]`, PostgreSQL `TEXT` `sqlx` encoding/decoding behind `feature = "sqlx"`, `Display`, `AsRef<str>`, `as_str(&self) -> &str`, `FromStr`, `TryFrom<String>` -> `BoundedTextError` (rejects `'\0'`, `SingleLine` rejects `'\r'` / `'\n'` before trimming while `MultiLine` permits internal newlines, trims whitespace, and enforces `(MIN..=MAX).contains(&trimmed.chars().count())`). Define `EmailSubject = BoundedText<1, 200, SingleLine>`, `EmailMessage = BoundedText<1, 5000, MultiLine>`, `ContactName = BoundedText<1, 120>` (with extension trait `ContactNameExt::from_email_local_part`), `ContactPhone = BoundedText<1, 40>`, `ContactNotes = BoundedText<0, 2000>`, and `CommentBody = BoundedText<1, 500>`.
- **Database reuse and single-query round-trips (`pp-database`):**
  - Share `Present::deserialize` (`Option<Option<T>>` tri-state patch deserialization) in `crates/database/src/patch.rs` across `ContactChanges` and `GalleryChanges`.
  - Share `ShareToken` unique-violation collision retry logic across `GalleryRepository` (`share_with` / `share_in`) and `ContactRepository` (`upsert_and_link_in` / `link_gallery_with`), and reuse `ContactRepository::find_gallery_link` inside `link_gallery_with`.
  - Collapse `ContactRepository::find_gallery_link` (`SELECT` joining `contacts`, `gallery_contacts`, and `galleries` into `GalleryContactLink`) and `ContactRepository::update` (`UPDATE contacts AS c ... RETURNING ...` with the correlated `galleries` JSONB subquery) from two SQL round-trips each to a single SQL query each.
- **API and configuration cleanup (`pp-api`, `pp-core`):**
  - Derive `Deserialize` (`#[serde(deny_unknown_fields)]`) on `InvitationContact` and `InvitationBatch` and `Serialize` on `QueuedInvitations`, deleting `InvitationContactInput` and `SendInvitationsResponse` in `crates/api/src/routes/galleries/invitations.rs`. Deduplicate `InvitationBatch` recipients once (`To` first in order, then `BCC` omitting seen emails) via `InvitationBatch::deduplicate(&mut self)` and `recipient_count(&self)` before checking `max_recipients_per_send` and calling `queue_invitations`.
  - Replace inline `matches!` checks in `StudioBranding::from(WorkspaceSettings)` (`crates/api/src/routes/galleries/shared.rs`) with `settings.signature.shows_name()` and `settings.signature.shows_logo()`.
  - Parse `SmtpConfig.password` as `Option<String>` (`hide_env_values = true`) and validate it into `SmtpPassword` inside `SmtpConfig::resolve()` alongside `host` and `from_name`, deleting `SmtpPasswordValueParser`.

## Non-goals

- Changing any HTTP route path, wire field name, release-build error `code` on live routes, or PostgreSQL schema/migration.
- Converting multi-rule domain types (`Email`, `GalleryAccessCode`, `GalleryTitle`, `WorkspaceName`) whose normalization or validation differs from bounded integer/trimmed-text semantics.

## Constraints

- Zero free functions in non-test Rust code (`crates/AGENTS.md`); use associated methods, generic types, and extension traits.
- Never echo `SmtpPassword` in `Debug`, `Display`, or CLI/config error messages (`crates/AGENTS.md`).

## Approaches

- **A. Const-generic `BoundedU8` and marker-parameterized `BoundedText<MIN, MAX, L>` (`SingleLine` / `MultiLine`) with `pub type` aliases and extension traits where needed (chosen).** Matches `ExportRetention = BoundedDuration<...>` and `crates/AGENTS.md` ("Prefer one generic type over several near-identical specific ones when only a type parameter differs" and "Replace boolean or stringly-typed flags with enums or config types"), uses zero macros, and deletes maximum wrapper boilerplate; all nine domain aliases have distinct `(MIN, MAX, L)` parameters.
- **B. Thin `#[serde(transparent)] #[sqlx(transparent)]` nominal wrapper structs around `BoundedU8` / `BoundedText`.** Preserves separate nominal struct types even if bounds ever collide, at the cost of ~6 lines of boilerplate per type; rejected.
- **C. `macro_rules!` generating bespoke newtype and error structs.** Preserves per-field `Display` strings that release API responses never expose, while violating `crates/AGENTS.md`'s preference for generic types over macros; rejected.

## Decisions

- **Maintainer decision:** Unify range/length parse errors on pure generic `BoundedU8` and `BoundedText` types with `pub type` aliases (Approach A) rather than `macro_rules!` or nominal wrapper structs.
- **Author's call — Dead settings error codes:** Remove unreachable `SettingsError::GridCell` / `SettingsError::SizeStep` and dead `ErrorCode::InvalidPosition` / `ErrorCode::InvalidSize` mappings in `pp-api` (out-of-range `u8` fields in `PUT /api/v1/settings` already fail during `SettingsRequest` JSON deserialization with `400 invalid_request_body`).
- **Author's call — `SmtpPassword` CLI parsing:** Keep `SmtpConfig` non-`Debug` with `#[arg(..., hide_env_values = true)] pub password: Option<String>` and construct `SmtpPassword` inside `SmtpConfig::resolve()`, so `clap` never rejects or echoes raw `PP_SMTP_PASSWORD` values while deleting `SmtpPasswordValueParser`.

## Left to the builder

- Exact method signatures on `BoundedU8`, `BoundedText`, `Present`, and the `ShareToken` collision-retry helper.
- Unit tests for `BoundedU8` / `BoundedText` edge cases (bounds, NUL bytes, single-line CR/LF before trim, whitespace trimming) and integration tests covering single-query `ContactRepository::{update, find_gallery_link}` and single-pass `InvitationBatch` deduplication.
