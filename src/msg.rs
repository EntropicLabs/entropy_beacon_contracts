use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const SUBMSG_REPLY_ID: u64 = 1;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct InstantiateMsg {
    pub deposit_fee: u64,
    pub key_activation_delay: u64,
    pub protocol_fee: u64,
    pub submitter_share: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct MigrateMsg {}
