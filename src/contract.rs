#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, BankMsg, Binary, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env, MessageInfo, Reply,
    ReplyOn, Response, StdError, StdResult, SubMsg, SubMsgResult, Uint128,
};
use cw2::set_contract_version;
use ecvrf::encode_hex;
use entropy_beacon_cosmos::{
    beacon::{RequestEntropyMsg, UpdateConfigMsg},
    msg::{ExecuteMsg, QueryMsg},
    provide::{
        ActiveRequestsResponse, KeyStatusQuery, KeyStatusResponse, LastEntropyResponse,
        SubmitEntropyMsg, WhitelistPublicKeyMsg,
    },
    EntropyCallbackMsg,
};

use crate::msg::{InstantiateMsg, SUBMSG_REPLY_ID};
use crate::state::{
    Config, EntropyRequest, State, ACTIVE_REQUESTS, CONFIG, STATE, WHITELISTED_KEYS,
};
use crate::utils::{check_key, is_whitelisted};
use crate::{error::ContractError, msg::MigrateMsg};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:entropy";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let state = State { last_entropy: None };
    let cfg = Config {
        owner: info.sender.clone(),
        deposit_fee: msg.deposit_fee,
        key_activation_delay: msg.key_activation_delay,
        protocol_fee: msg.protocol_fee,
        submitter_share: Decimal::percent(msg.submitter_share),
    };
    STATE.save(deps.storage, &state)?;
    CONFIG.save(deps.storage, &cfg)?;

    ACTIVE_REQUESTS.save(deps.storage, &vec![])?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("owner", info.sender)
        .add_attribute(
            "key_activation_delay",
            format!("{}", msg.key_activation_delay),
        ))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateConfig(data) => update_config(deps, env, info, data),
        ExecuteMsg::WhitelistPublicKey(data) => whitelist_key(deps, env, info, data),
        ExecuteMsg::SubmitEntropy(data) => submit_entropy(deps, env, info, data),
        ExecuteMsg::RequestEntropy(data) => request_entropy(deps, env, info, data),
    }
}

/// Update the configuration of the contract
/// This is only allowed to be called by the owner
fn update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    data: UpdateConfigMsg,
) -> Result<Response, ContractError> {
    let mut cfg = CONFIG.load(deps.storage)?;
    if cfg.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    cfg.deposit_fee = data.deposit_fee;
    cfg.key_activation_delay = data.key_activation_delay;
    cfg.protocol_fee = data.protocol_fee;
    cfg.submitter_share = Decimal::percent(data.submitter_share);

    CONFIG.save(deps.storage, &cfg)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

/// Whitelists a public key, noting down the block height.
/// A public key can only submit entropy after a "key activation delay"
/// period has passed. See `crate::state::Config::key_activation_delay`.
fn whitelist_key(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    data: WhitelistPublicKeyMsg,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    let key = data.public_key;
    if key.validate().is_err() {
        return Err(ContractError::InvalidPublicKey {});
    }

    if is_whitelisted(&deps.as_ref(), &key) {
        return Err(ContractError::KeyAlreadyWhitelisted {});
    }

    let received_funds_amt: Uint128 = info.funds.iter().map(|c| c.amount).sum();
    if received_funds_amt < Uint128::from(cfg.deposit_fee) {
        return Err(ContractError::InsufficientFunds {});
    }

    WHITELISTED_KEYS.save(deps.storage, key.as_bytes(), &env.block.height)?;
    Ok(Response::new()
        .add_attribute("action", "whitelist_public_key")
        .add_attribute("public_key", format!("{}", data.public_key))
        .add_attribute(
            "activation_height",
            format!("{}", env.block.height + cfg.key_activation_delay),
        ))
}

/// Allows a public key holder to submit entropy through the VRF proof.
/// Then, forwards the entropy to all pending requests through their
/// callback methods. Errors in callback methods will not cancel the
/// entire transaction.
///
/// Ensures that the public key that is used has been whitelisted for
/// a minimum of `key_activation_delay` blocks, to prevent attacks.
/// Also ensures that the message/seed used for the VRF proof is the
/// entropy that was last submitted.
fn submit_entropy(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    data: SubmitEntropyMsg,
) -> Result<Response, ContractError> {
    let mut state = STATE.load(deps.storage)?;
    let cfg = CONFIG.load(deps.storage)?;
    let proof = data.proof;
    if let Some(last) = state.last_entropy {
        if last != proof.message_bytes {
            return Err(ContractError::InvalidMessage {});
        }
    }

    let key = &proof.signer;
    check_key(&deps.as_ref(), &env, key, &cfg)?;

    let entropy = proof.verify().map_err(|_| ContractError::InvalidProof {})?;

    state.last_entropy = Some(entropy.to_vec());
    STATE.save(deps.storage, &state)?;

    let requests = ACTIVE_REQUESTS.load(deps.storage)?;
    //TODO the payout should have a nominal amount deducted from it for a protocol fee.
    let payout: Uint128 = requests.iter().map(|req| req.submitted_bounty_amount).sum();
    let mut submsgs = vec![];
    for req in requests {
        submsgs.push(SubMsg {
            id: SUBMSG_REPLY_ID,
            msg: EntropyCallbackMsg {
                entropy: entropy.to_vec(),
                msg: req.callback_msg,
            }
            .into_cosmos_msg(req.callback_address)?, //TODO: validate the callback address, maybe in request_entropy?
            gas_limit: Some(req.callback_gas_limit),
            reply_on: ReplyOn::Always,
        });
    }
    ACTIVE_REQUESTS.save(deps.storage, &vec![])?;
    let mut response = Response::new();
    if !payout.is_zero() {
        response = response.add_message(CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![Coin {
                denom: "uluna".to_string(),
                amount: payout,
            }],
        }));
    }

    Ok(response
        .add_submessages(submsgs)
        .add_attribute("action", "submit_entropy")
        .add_attribute("entropy", encode_hex(&entropy)))
}

/// Allows any smart contract to request entropy from the beacon.
/// Ensures that the caller has provided enough funds to pay both
/// the requested callback gas and the protocol fee.
fn request_entropy(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    data: RequestEntropyMsg,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    let received_funds_amt: Uint128 = info.funds.iter().map(|c| c.amount).sum();
    if received_funds_amt < Uint128::from(data.callback_gas_limit + cfg.protocol_fee) {
        return Err(ContractError::InsufficientFunds {});
    }

    let request = EntropyRequest {
        callback_gas_limit: data.callback_gas_limit,
        callback_address: data.callback_address,
        callback_msg: data.callback_msg,
        submitter: info.sender,
        submitted_block_height: env.block.height,
        submitted_bounty_amount: received_funds_amt,
    };
    let mut active_requests = ACTIVE_REQUESTS.load(deps.storage)?;
    active_requests.push(request);
    ACTIVE_REQUESTS.save(deps.storage, &active_requests)?;
    Ok(Response::new()
        .add_attribute("action", "request_entropy")
        .add_attribute("request_id", format!("{}", active_requests.len() - 1)))
}

/// Handles the reply of submessage calls. If the call was an error,
/// forwards the message through a status.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(_deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    if msg.id != SUBMSG_REPLY_ID {
        return Err(ContractError::InvalidReplyId {});
    }

    Ok(Response::new()
        .add_attribute("action", "callback_reply")
        .add_attribute(
            "callback_status",
            match msg.result {
                SubMsgResult::Ok(_) => "ok".to_string(),
                SubMsgResult::Err(e) => format!("Error: {}", e),
            },
        ))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::KeyStatus(data) => to_binary(&key_status_query(deps, env, data)?),
        QueryMsg::LastEntropy(_) => to_binary(&last_entropy_query(deps)?),
        QueryMsg::ActiveRequests(_) => to_binary(&active_requests_query(deps)?),
    }
}

/// Checks whether a key is whitelisted, and if so, whether enough blocks
/// have passed for the key to be used to submit entropy.
fn key_status_query(deps: Deps, env: Env, data: KeyStatusQuery) -> StdResult<KeyStatusResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    let key = data.public_key;

    if key.validate().is_err() {
        return Err(StdError::GenericErr {
            msg: "Invalid public key".to_string(),
        });
    }

    let status = check_key(&deps, &env, &key, &cfg);
    match status {
        Ok(h) => Ok(KeyStatusResponse {
            whitelisted: true,
            active: true,
            activation_height: h,
        }),
        Err(ContractError::KeyNotWhitelisted { .. }) => Ok(KeyStatusResponse {
            whitelisted: false,
            active: false,
            activation_height: 0,
        }),
        Err(ContractError::KeyNotActive { activation_height }) => Ok(KeyStatusResponse {
            whitelisted: true,
            active: false,
            activation_height,
        }),
        Err(_) => Err(StdError::GenericErr {
            msg: "Unexpected error".to_string(),
        }),
    }
}

/// Returns the last submitted entropy, as a hex string.
fn last_entropy_query(deps: Deps) -> StdResult<LastEntropyResponse> {
    let cfg = STATE.load(deps.storage)?;
    let last = cfg.last_entropy.unwrap_or_default();
    Ok(LastEntropyResponse {
        entropy: encode_hex(last.as_slice()),
    })
}

fn active_requests_query(deps: Deps) -> StdResult<ActiveRequestsResponse> {
    let requests = ACTIVE_REQUESTS.load(deps.storage)?;
    Ok(ActiveRequestsResponse {
        bounties: requests
            .iter()
            .map(|req| req.submitted_bounty_amount)
            .collect(),
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    Ok(Response::new().add_attribute("action", "migrate"))
}
