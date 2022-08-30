use cosmwasm_std::{Uint128, Addr};
use ecvrf_rs::PublicKey;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const SUBMSG_REPLY_ID: u64 = 1;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct InstantiateMsg {
    pub whitelist_deposit_amt: Uint128,
    pub refund_increment_amt: Uint128,
    pub key_activation_delay: u64,
    pub protocol_fee: u64,
    pub submitter_share: u64,
    pub native_denom: String,
    pub whitelisted_keys: Vec<(Addr, PublicKey)>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct MigrateMsg {}
