#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, BankMsg, Binary, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env, MessageInfo, Reply,
    ReplyOn, Response, StdError, StdResult, SubMsg, SubMsgResult, Uint128,
};
use cw2::set_contract_version;
use ecvrf_rs::encode_hex;
use entropy_beacon_cosmos::{
    beacon::{calculate_gas_cost, RequestEntropyMsg, UpdateConfigMsg},
    msg::{ExecuteMsg, QueryMsg},
    provide::{
        ActiveRequestsResponse, AdminReturnDepositMsg, BeaconConfigResponse, KeyStatusQuery,
        KeyStatusResponse, LastEntropyResponse, ReclaimDepositMsg, SubmitEntropyMsg,
        WhitelistPublicKeyMsg,
    },
    EntropyCallbackMsg,
};
use sha2::{Digest, Sha512};

use crate::state::{
    Config, EntropyRequest, State, ACTIVE_REQUESTS, CONFIG, STATE, WHITELISTED_KEYS,
};
use crate::utils::{check_key, is_whitelisted};
use crate::{error::ContractError, msg::MigrateMsg};
use crate::{
    msg::{InstantiateMsg, SUBMSG_REPLY_ID},
    state::KeyInfo,
};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:entropy";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let state = State { last_entropy: None };
    let cfg = Config {
        owner: info.sender.clone(),
        whitelist_deposit_amt: msg.whitelist_deposit_amt,
        refund_increment_amt: msg.refund_increment_amt,
        key_activation_delay: msg.key_activation_delay,
        protocol_fee: msg.protocol_fee,
        submitter_share: Decimal::percent(msg.submitter_share),
        native_denom: msg.native_denom,
    };
    STATE.save(deps.storage, &state)?;
    CONFIG.save(deps.storage, &cfg)?;

    ACTIVE_REQUESTS.save(deps.storage, &vec![])?;

    for (addr, key) in msg.whitelisted_keys {
        if key.validate().is_err() {
            return Err(ContractError::InvalidPublicKey {});
        }
        WHITELISTED_KEYS.save(
            deps.storage,
            key.as_bytes(),
            &KeyInfo {
                holder: addr,
                deposit_amount: Uint128::zero(),
                refundable_amount: Uint128::zero(),
                creation_height: env.block.height,
            },
        )?;
    }

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("owner", info.sender))
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
        ExecuteMsg::ReclaimDeposit(data) => reclaim_deposit(deps, env, info, data),
        ExecuteMsg::SubmitEntropy(data) => submit_entropy(deps, env, info, data),
        ExecuteMsg::RequestEntropy(data) => request_entropy(deps, env, info, data),
        ExecuteMsg::AdminReturnDeposit(data) => admin_return_deposit(deps, env, info, data),
    }
}

pub fn admin_return_deposit(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    data: AdminReturnDepositMsg,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if cfg.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }
    if !is_whitelisted(&deps.as_ref(), &data.key) {
        return Err(ContractError::KeyNotWhitelisted {});
    }
    let key_info = WHITELISTED_KEYS.load(deps.storage, data.key.as_bytes())?;
    WHITELISTED_KEYS.remove(deps.storage, data.key.as_bytes());
    Ok(Response::new()
        .add_message(CosmosMsg::Bank(BankMsg::Send {
            to_address: key_info.holder.to_string(),
            amount: vec![Coin {
                denom: "uluna".to_string(),
                amount: key_info.deposit_amount,
            }],
        }))
        .add_attribute("action", "reclaim_deposit")
        .add_attribute("unwhitelisted_key", format!("{}", data.key))
        .add_attribute("refund", format!("{}", key_info.deposit_amount)))
}

/// Update the configuration of the contract
/// This is only allowed to be called by the owner
pub fn update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    data: UpdateConfigMsg,
) -> Result<Response, ContractError> {
    let mut cfg = CONFIG.load(deps.storage)?;
    if cfg.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    cfg.whitelist_deposit_amt = data.whitelist_deposit_amt;
    cfg.key_activation_delay = data.key_activation_delay;
    cfg.protocol_fee = data.protocol_fee;
    cfg.submitter_share = Decimal::percent(data.submitter_share);

    CONFIG.save(deps.storage, &cfg)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

/// Whitelists a public key, noting down the block height.
/// A public key can only submit entropy after a "key activation delay"
/// period has passed. See `crate::state::Config::key_activation_delay`.
pub fn whitelist_key(
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

    let received_funds_amt: Uint128 = info
        .funds
        .iter()
        .filter_map(|coin| {
            if coin.denom == cfg.native_denom {
                Some(coin.amount)
            } else {
                None
            }
        })
        .sum();
    if received_funds_amt < cfg.whitelist_deposit_amt {
        return Err(ContractError::InsufficientFunds {});
    }

    WHITELISTED_KEYS.save(
        deps.storage,
        key.as_bytes(),
        &KeyInfo {
            creation_height: env.block.height,
            deposit_amount: cfg.whitelist_deposit_amt,
            refundable_amount: Uint128::zero(),
            holder: info.sender,
        },
    )?;
    Ok(Response::new()
        .add_attribute("action", "whitelist_public_key")
        .add_attribute("public_key", format!("{}", data.public_key))
        .add_attribute(
            "activation_height",
            format!("{}", env.block.height + cfg.key_activation_delay),
        ))
}

/// Allows the holder of a public key to unwhitelist it, and reclaim the
/// deposit that was made when the key was whitelisted.
pub fn reclaim_deposit(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    data: ReclaimDepositMsg,
) -> Result<Response, ContractError> {
    let key = data.public_key;

    if !is_whitelisted(&deps.as_ref(), &key) {
        return Err(ContractError::KeyNotWhitelisted {});
    }
    let key_info = WHITELISTED_KEYS.load(deps.storage, key.as_bytes())?;

    if info.sender != key_info.holder {
        return Err(ContractError::Unauthorized {});
    }

    WHITELISTED_KEYS.remove(deps.storage, key.as_bytes());

    let refund_amt = key_info.refundable_amount.min(key_info.deposit_amount);

    Ok(Response::new()
        .add_message(CosmosMsg::Bank(BankMsg::Send {
            to_address: key_info.holder.to_string(),
            amount: vec![Coin {
                denom: "uluna".to_string(),
                amount: refund_amt,
            }],
        }))
        .add_attribute("action", "reclaim_deposit")
        .add_attribute("unwhitelisted_key", format!("{}", key))
        .add_attribute("refund", format!("{}", refund_amt)))
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
pub fn submit_entropy(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    data: SubmitEntropyMsg,
) -> Result<Response, ContractError> {
    let mut state = STATE.load(deps.storage)?;
    let cfg = CONFIG.load(deps.storage)?;
    let proof = data.proof;
    let requests = ACTIVE_REQUESTS.load(deps.storage)?;

    if requests.is_empty() {
        return Err(ContractError::NoActiveRequests {});
    }

    if state.last_entropy.unwrap_or_default() != proof.message_bytes {
        return Err(ContractError::InvalidMessage {});
    }

    let key = &proof.signer;
    check_key(&deps.as_ref(), &env, key, &cfg)?;

    let mut key_info = WHITELISTED_KEYS.load(deps.storage, key.as_bytes())?;
    if key_info.holder != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    let entropy = proof.verify().map_err(|_| ContractError::InvalidProof {})?;

    state.last_entropy = Some(entropy.to_vec());
    STATE.save(deps.storage, &state)?;

    key_info.refundable_amount =
        (key_info.refundable_amount + cfg.refund_increment_amt).min(key_info.deposit_amount);
    WHITELISTED_KEYS.save(deps.storage, key.as_bytes(), &key_info)?;

    let payout: Uint128 = requests.iter().map(|req| req.submitted_bounty_amount).sum();
    let payout = payout * cfg.submitter_share;
    let mut submsgs = vec![];

    let mut cur_entropy = entropy;
    for req in requests {
        submsgs.push(SubMsg {
            id: SUBMSG_REPLY_ID,
            msg: EntropyCallbackMsg {
                entropy: cur_entropy.to_vec(),
                requester: req.submitter,
                msg: req.callback_msg,
            }
            .into_cosmos_msg(req.callback_address)?,
            gas_limit: Some(req.callback_gas_limit),
            reply_on: ReplyOn::Always,
        });

        let mut hasher = Sha512::new();
        hasher.update(&cur_entropy);
        cur_entropy = hasher.finalize().into();
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
pub fn request_entropy(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    data: RequestEntropyMsg,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    let received_funds_amt: Uint128 = info
        .funds
        .iter()
        .filter(|c| c.denom == cfg.native_denom)
        .map(|c| c.amount)
        .sum();
    let gas_cost = calculate_gas_cost(data.callback_gas_limit);
    let protocol_fee = Uint128::from(cfg.protocol_fee);

    if received_funds_amt < gas_cost + protocol_fee {
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
    match msg.result {
        SubMsgResult::Ok(_) => Ok(Response::new()),
        SubMsgResult::Err(e) => Ok(Response::new()
            .set_data(e.as_bytes())
            .add_attribute("action", "reply_error")
            .add_attribute("error", e)),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::KeyStatus(data) => to_binary(&key_status_query(deps, env, data)?),
        QueryMsg::LastEntropy(_) => to_binary(&last_entropy_query(deps)?),
        QueryMsg::ActiveRequests(_) => to_binary(&active_requests_query(deps)?),
        QueryMsg::BeaconConfig(_) => to_binary(&beacon_config_query(deps)?),
    }
}

/// Checks whether a key is whitelisted, and if so, whether enough blocks
/// have passed for the key to be used to submit entropy.
pub fn key_status_query(
    deps: Deps,
    env: Env,
    data: KeyStatusQuery,
) -> StdResult<KeyStatusResponse> {
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
pub fn last_entropy_query(deps: Deps) -> StdResult<LastEntropyResponse> {
    let cfg = STATE.load(deps.storage)?;
    let last = cfg.last_entropy.unwrap_or_default();
    Ok(LastEntropyResponse {
        entropy: encode_hex(last.as_slice()),
    })
}

pub fn active_requests_query(deps: Deps) -> StdResult<ActiveRequestsResponse> {
    let requests = ACTIVE_REQUESTS.load(deps.storage)?;
    let requests = requests.iter().map(|r| r.clone().into_info()).collect();
    Ok(ActiveRequestsResponse { requests })
}

pub fn beacon_config_query(deps: Deps) -> StdResult<BeaconConfigResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    Ok(BeaconConfigResponse {
        whitelist_deposit_amt: cfg.whitelist_deposit_amt,
        key_activation_delay: cfg.key_activation_delay,
        protocol_fee: cfg.protocol_fee,
        submitter_share: cfg.submitter_share,
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    Ok(Response::new().add_attribute("action", "migrate"))
}
