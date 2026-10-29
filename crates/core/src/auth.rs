use derive_more::{Display, From};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, From)]
#[serde(transparent)]
pub struct AccountId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, From)]
#[serde(transparent)]
pub struct WorkspaceId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub account_id: AccountId,
    pub workspace_id: WorkspaceId,
}
