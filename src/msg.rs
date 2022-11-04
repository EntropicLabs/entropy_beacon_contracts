use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const SUBMSG_REPLY_ID: u64 = 1;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct MigrateMsg {}
