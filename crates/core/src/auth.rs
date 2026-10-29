use derive_more::{Display, From};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Identifies the single photographer account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, From)]
#[serde(transparent)]
pub struct AccountId(pub Uuid);

/// Owner of galleries and settings. Kept separate from [`AccountId`] so collaborators can
/// later join a workspace without changing who owns its data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, From)]
#[serde(transparent)]
pub struct WorkspaceId(pub Uuid);

/// Who a valid session belongs to; the only thing protected operations should trust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub account_id: AccountId,
    pub workspace_id: WorkspaceId,
}
