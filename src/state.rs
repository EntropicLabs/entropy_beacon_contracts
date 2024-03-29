use cosmwasm_schema::cw_serde;
use entropy_beacon_cosmos::{provide::ActiveRequestInfo, BeaconConfigResponse};

use cosmwasm_std::{Addr, Binary, Decimal, Uint128};
use cw_storage_plus::{Item, Map};

#[cw_serde]
pub struct State {
    ///The last submitted entropy.
    pub last_entropy: Option<Vec<u8>>,
    ///Currently believed gas price.
    pub belief_gas_price: Decimal,
    ///Current request id counter.
    pub cur_request_id: u128,
}
#[cw_serde]
pub struct Config {
    pub owner: Addr,
    ///The amount of tokens that must be deposited to whitelist a new public key.
    pub whitelist_deposit_amt: Uint128,
    ///The amount of the deposit that unlocks with each submission of entropy.
    pub refund_increment_amt: Uint128,
    ///The time, in blocks, before a whitelisted public key can be used to submit entropy.
    pub key_activation_delay: u64,
    ///The fee that the protocol contract charges on top of the requested gas fees.
    pub protocol_fee: u64,
    ///The share of the protocol fee that is distributed to the wallet submitting entropy.
    pub submitter_share: Decimal,
    ///The native currency of the target chain.
    pub native_denom: String,
    ///Whether or not the contract is paused.
    pub paused: bool,
    ///Whether or not the contract is in permissioned mode.
    pub permissioned: bool,
    ///Whether or not the contract is in test mode.
    pub test_mode: bool,
    ///Whether or not callback subsidization is enabled.
    pub subsidize_callbacks: bool,
}

impl From<Config> for BeaconConfigResponse {
    fn from(val: Config) -> Self {
        BeaconConfigResponse {
            whitelist_deposit_amt: val.whitelist_deposit_amt,
            refund_increment_amt: val.refund_increment_amt,
            key_activation_delay: val.key_activation_delay,
            protocol_fee: val.protocol_fee,
            submitter_share: val.submitter_share,
            native_denom: val.native_denom,
            paused: val.paused,
            permissioned: val.permissioned,
            test_mode: val.test_mode,
            subsidize_callbacks: val.subsidize_callbacks,
        }
    }
}

#[cw_serde]
pub struct EntropyRequest {
    ///The id of the request.
    pub id: u128,
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

impl EntropyRequest {
    pub fn into_info(self) -> ActiveRequestInfo {
        ActiveRequestInfo {
            id: Uint128::from(self.id),
            callback_gas_limit: self.callback_gas_limit,
            callback_address: self.callback_address,
            submitter: self.submitter,
            submitted_block_height: self.submitted_block_height,
            submitted_bounty_amount: self.submitted_bounty_amount,
        }
    }
}

#[cw_serde]
pub struct KeyInfo {
    pub holder: Addr,
    pub deposit_amount: Uint128,
    pub refundable_amount: Uint128,
    pub creation_height: u64,
}

pub const STATE: Item<State> = Item::new("state");
pub const CONFIG: Item<Config> = Item::new("config");
pub const WHITELISTED_KEYS: Map<&[u8], KeyInfo> = Map::new("whitelisted_keys");

pub const ENTROPY_REQUESTS: Map<u128, EntropyRequest> = Map::new("entropy_requests");