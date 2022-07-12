use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Binary, Decimal, Uint128};
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct State {
    ///The last submitted entropy.
    pub last_entropy: Option<Vec<u8>>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Config {
    pub owner: Addr,
    ///The amount of tokens that must be deposited to whitelist a new public key.
    pub whitelist_deposit_amt: Uint128,
    ///The time, in blocks, before a whitelisted public key can be used to submit entropy.
    pub key_activation_delay: u64,
    ///The fee that the protocol contract charges on top of the requested gas fees.
    pub protocol_fee: u64,
    ///The share of the protocol fee that is distributed to the wallet submitting entropy.
    pub submitter_share: Decimal,
    ///The native currency of the target chain.
    pub native_denom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct EntropyRequest {
    ///How much gas the requester has provisioned for their callback transaction.
    pub callback_gas_limit: u64,
    ///The address to send the callback message to.
    pub callback_address: Addr,
    ///A custom callback message that was included by the requester.
    pub callback_msg: Binary,

    ///The address that we received the request from.
    pub submitter: Addr,
    ///The block that the request was received on.
    pub submitted_block_height: u64,
    ///The amount of tokens left after subtracting the requested gas.
    pub submitted_bounty_amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct KeyInfo {
    pub holder: Addr,
    pub deposit_amount: Uint128,
    pub creation_height: u64,
}

pub const STATE: Item<State> = Item::new("state");
pub const CONFIG: Item<Config> = Item::new("config");
pub const WHITELISTED_KEYS: Map<&[u8], KeyInfo> = Map::new("whitelisted_keys");
pub const ACTIVE_REQUESTS: Item<Vec<EntropyRequest>> = Item::new("active_requests");
