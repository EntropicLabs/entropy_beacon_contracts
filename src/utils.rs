use cosmwasm_std::{Deps, Env};
use ecvrf_rs::PublicKey;

use crate::{
    state::{WHITELISTED_KEYS, Config},
    ContractError,
};

/// Does a simple check to see whether a key is in the whitelisted.
/// IMPORTANT: This function does NOT check for key activation status
pub fn is_whitelisted(deps: &Deps, key: &PublicKey) -> bool {
    WHITELISTED_KEYS.has(deps.storage, key.as_bytes())
}

/// Checks whether a key is unwhitelisted, or pending activation.
/// If the key is valid, returns the block height at which it was activated.
pub fn check_key(
    deps: &Deps,
    env: &Env,
    key: &PublicKey,
    cfg: &Config,
) -> Result<u64, ContractError> {
    let created_time = WHITELISTED_KEYS
        .load(deps.storage, key.as_bytes())
        .map_err(|_| ContractError::KeyNotWhitelisted {})?.creation_height;
    if created_time + cfg.key_activation_delay <= env.block.height {
        Ok(created_time + cfg.key_activation_delay)
    } else {
        Err(ContractError::KeyNotActive {
            activation_height: created_time + cfg.key_activation_delay,
        })
    }
}
