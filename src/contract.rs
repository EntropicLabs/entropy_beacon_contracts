#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdError,
    StdResult, SubMsgResult, Uint128,
};
use cw2::set_contract_version;
use entropy_beacon_cosmos::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};

use crate::{error::ContractError, msg::MigrateMsg, query};
use crate::{
    execute,
    state::{Config, State, CONFIG, STATE, WHITELISTED_KEYS},
};
use crate::{msg::SUBMSG_REPLY_ID, state::KeyInfo};

// version info for migration info
const CONTRACT_NAME: &str = "entropiclabs/beacon";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let state = State {
        last_entropy: None,
        belief_gas_price: msg.belief_gas_price,
        cur_request_id: 0u128,
    };

    let cfg = Config {
        owner: info.sender.clone(),
        whitelist_deposit_amt: msg.whitelist_deposit_amt,
        refund_increment_amt: msg.refund_increment_amt,
        key_activation_delay: msg.key_activation_delay,
        protocol_fee: msg.protocol_fee,
        submitter_share: Decimal::percent(msg.submitter_share),
        native_denom: msg.native_denom,
        paused: false,
        permissioned: msg.permissioned,
        test_mode: msg.test_mode,
    };

    STATE.save(deps.storage, &state)?;
    CONFIG.save(deps.storage, &cfg)?;

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
        ExecuteMsg::UpdateConfig(data) => execute::update_config(deps, env, info, data),
        ExecuteMsg::WhitelistPublicKey(data) => execute::whitelist_key(deps, env, info, data),
        ExecuteMsg::ReclaimDeposit(data) => execute::reclaim_deposit(deps, env, info, data),
        ExecuteMsg::SubmitEntropy(data) => execute::submit_entropy(deps, env, info, data),
        ExecuteMsg::RequestEntropy(data) => execute::request_entropy(deps, env, info, data),
        ExecuteMsg::AdminReturnDeposit(data) => {
            execute::admin_return_deposit(deps, env, info, data)
        }
        ExecuteMsg::UpdateGasPrice(gas_price) => {
            execute::update_gas_price(deps, env, info, gas_price)
        }
    }
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
        QueryMsg::KeyStatus(data) => to_binary(&query::key_status_query(deps, env, data)?),
        QueryMsg::LastEntropy(_) => to_binary(&query::last_entropy_query(deps)?),
        QueryMsg::ActiveRequests(data) => to_binary(&query::active_requests_query(deps, data)?),
        QueryMsg::BeaconConfig(_) => to_binary(&query::beacon_config_query(deps)?),
        QueryMsg::CalculateFee(data) => to_binary(&query::calculate_fee_query(deps, data)?),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    let version = cw2::get_contract_version(deps.storage)?;

    if !version.version.starts_with('1') && !version.version.starts_with('0') {
        return Err(ContractError::Std(StdError::generic_err(
            format!("Invalid version for migration: {}", version.version),
        )));
    }

    let v1_state = crate::state::v1::STATE.load(deps.storage)?;
    let v1_config = crate::state::v1::CONFIG.load(deps.storage)?;

    let state = State {
        last_entropy: v1_state.last_entropy,
        belief_gas_price: msg.belief_gas_price,
        cur_request_id: 0u128,
    };

    STATE.save(deps.storage, &state)?;

    let config = Config {
        owner: v1_config.owner,
        whitelist_deposit_amt: v1_config.whitelist_deposit_amt,
        refund_increment_amt: v1_config.refund_increment_amt,
        key_activation_delay: v1_config.key_activation_delay,
        protocol_fee: v1_config.protocol_fee,
        submitter_share: v1_config.submitter_share,
        native_denom: v1_config.native_denom,
        paused: false,
        permissioned: true,
        test_mode: false,
    };

    CONFIG.save(deps.storage, &config)?;

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::new().add_attribute("action", "migrate"))
}
