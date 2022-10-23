use cosmwasm_std::{Deps, Env, Order, StdError, StdResult, Uint128};
use cw_storage_plus::Bound;
use ecvrf_rs::encode_hex;
use entropy_beacon_cosmos::{
    beacon::{CalculateFeeQuery, CalculateFeeResponse},
    provide::{
        ActiveRequestsQuery, ActiveRequestsResponse, BeaconConfigResponse, KeyStatusQuery,
        KeyStatusResponse, LastEntropyResponse, DEFAULT_PAGINATION_LIMIT, MAX_PAGINATION_LIMIT,
    },
};

use crate::state::{CONFIG, STATE};
use crate::utils::check_key;
use crate::{error::ContractError, state::ENTROPY_REQUESTS};

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

pub fn active_requests_query(
    deps: Deps,
    data: ActiveRequestsQuery,
) -> StdResult<ActiveRequestsResponse> {
    let limit = data
        .limit
        .unwrap_or(DEFAULT_PAGINATION_LIMIT)
        .min(MAX_PAGINATION_LIMIT) as u128;

    let start = data.start_after.map(Bound::exclusive);
    let end = match data.start_after {
        Some(start_after) => Some(Bound::inclusive(start_after + limit)),
        None => Some(Bound::exclusive(limit)),
    };

    let requests: Vec<_> = ENTROPY_REQUESTS
        .range(deps.storage, start, end, Order::Ascending)
        .map(|item| item.map(|(_, r)| r.into_info()))
        .collect::<StdResult<_>>()?;

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

pub fn calculate_fee_query(deps: Deps, data: CalculateFeeQuery) -> StdResult<CalculateFeeResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let gas_cost = Uint128::from(data.callback_gas_limit) * state.belief_gas_price;
    let protocol_fee = Uint128::from(cfg.protocol_fee);
    let total_fee = gas_cost + protocol_fee;
    if total_fee > u64::MAX.into() {
        return Err(StdError::generic_err("Fee overflow"));
    }
    let total_fee = total_fee.u128() as u64;
    Ok(CalculateFeeResponse {
        fee: total_fee,
        gas_price: state.belief_gas_price,
    })
}
