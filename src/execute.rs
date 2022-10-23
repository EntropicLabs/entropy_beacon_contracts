use cosmwasm_std::{
    BankMsg, Coin, CosmosMsg, Decimal, DepsMut, Env, MessageInfo, Order, ReplyOn, Response, SubMsg,
    Uint128, StdError,
};
use ecvrf_rs::encode_hex;
use entropy_beacon_cosmos::{
    beacon::{CalculateFeeQuery, RequestEntropyMsg, UpdateConfigMsg},
    provide::{AdminReturnDepositMsg, ReclaimDepositMsg, SubmitEntropyMsg, WhitelistPublicKeyMsg},
    EntropyCallbackMsg,
};
use sha2::{Digest, Sha512};

use crate::utils::{check_key, is_whitelisted};
use crate::{error::ContractError, state::ENTROPY_REQUESTS};
use crate::{msg::SUBMSG_REPLY_ID, state::KeyInfo};
use crate::{
    query,
    state::{EntropyRequest, CONFIG, STATE, WHITELISTED_KEYS},
};
pub fn update_gas_price(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    gas_price: Decimal,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if cfg.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }
    let mut state = STATE.load(deps.storage)?;
    state.belief_gas_price = gas_price;
    STATE.save(deps.storage, &state)?;
    Ok(Response::new()
        .add_attribute("action", "update_gas_price")
        .add_attribute("gas_price", gas_price.to_string()))
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
                denom: cfg.native_denom,
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
    cfg.refund_increment_amt = data.refund_increment_amt;
    cfg.protocol_fee = data.protocol_fee;
    cfg.submitter_share = Decimal::percent(data.submitter_share);
    cfg.permissioned = data.permissioned;

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
    if cfg.permissioned && cfg.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }

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
    let cfg = CONFIG.load(deps.storage)?;
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
                denom: cfg.native_denom,
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
    let request_ids = data.request_ids;

    let requests = if !request_ids.is_empty() {
        request_ids
            .iter()
            .map(|id| {
                let req = ENTROPY_REQUESTS.load(deps.storage, *id).map_err(
                    |e| match e {
                        StdError::NotFound { .. } => {
                            ContractError::NoMatchingRequests { request_id: *id }
                        }
                        _ => ContractError::Std(e),
                    },
                )?;
                Ok((*id, req))
            })
            .collect::<Result<Vec<_>, ContractError>>()?
    } else {
        ENTROPY_REQUESTS
            .range(deps.storage, None, None, Order::Ascending)
            .map(|item| {
                let (id, req) = item?;
                Ok((id, req))
            })
            .collect::<Result<Vec<_>, ContractError>>()?
    };

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

    let payout: Uint128 = requests.iter().map(|(_,req)| req.submitted_bounty_amount).sum();
    let payout = payout * cfg.submitter_share;
    let mut submsgs = vec![];

    let mut cur_entropy = entropy;
    for (id, req) in requests {
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

        ENTROPY_REQUESTS.remove(deps.storage, id);
    }
    
    let mut response = Response::new();
    if !payout.is_zero() {
        response = response.add_message(CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![Coin {
                denom: cfg.native_denom,
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
    let mut state = STATE.load(deps.storage)?;

    let received_funds_amt: Uint128 = info
        .funds
        .iter()
        .filter(|c| c.denom == cfg.native_denom)
        .map(|c| c.amount)
        .sum();

    let required_funds = query::calculate_fee_query(
        deps.as_ref(),
        CalculateFeeQuery {
            callback_gas_limit: data.callback_gas_limit,
        },
    )?;

    if received_funds_amt < Uint128::from(required_funds.fee) {
        return Err(ContractError::InsufficientFunds {});
    }

    let request_id = state.cur_request_id;

    let request = EntropyRequest {
        id: request_id,
        callback_gas_limit: data.callback_gas_limit,
        callback_address: data.callback_address,
        callback_msg: data.callback_msg,
        submitter: info.sender,
        submitted_block_height: env.block.height,
        submitted_bounty_amount: received_funds_amt,
    };

    ENTROPY_REQUESTS.save(deps.storage, request_id, &request)?;

    state.cur_request_id += 1;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("action", "request_entropy")
        .add_attribute("request_id", format!("{}", request_id)))
}
