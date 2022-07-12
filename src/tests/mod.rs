use cosmwasm_std::{
    testing::{mock_env, mock_info},
    DepsMut, Response, Uint128,
};
use ecvrf::PublicKey;

use crate::{contract::instantiate, msg::InstantiateMsg};

mod test_instantiate;
mod test_reclaim_deposit;
mod test_request_entropy;
mod test_submit_entropy;
mod test_whitelist_key;

pub fn test_pk() -> PublicKey {
    let pk =
        hex::decode("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a").unwrap();
    PublicKey::from_bytes(pk.as_slice())
}

pub fn default_instantiate(deps: DepsMut) -> Response {
    let msg = InstantiateMsg {
        whitelist_deposit_amt: Uint128::from(1000u128),
        key_activation_delay: 1,
        protocol_fee: 2,
        submitter_share: 80,
        native_denom: "uluna".to_string(),
    };
    let env = mock_env();
    let info = mock_info("creator", vec![].as_slice());

    // we can just call .unwrap() to assert this was a success
    instantiate(deps, env, info, msg).unwrap()
}