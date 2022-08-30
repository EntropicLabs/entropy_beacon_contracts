use cosmwasm_std::{
    coin,
    testing::{mock_dependencies, mock_env, mock_info},
};
use ecvrf_rs::PublicKey;
use entropy_beacon_cosmos::provide::{KeyStatusQuery, WhitelistPublicKeyMsg};

use crate::{
    contract::{key_status_query, whitelist_key},
    tests::{default_instantiate, test_pk},
    ContractError,
};

#[test]
fn whitelists_correctly() {
    let mut deps = mock_dependencies();
    default_instantiate(deps.as_mut());

    let env = mock_env();
    let info = mock_info("executor", &[coin(1000, "uluna")]);

    let msg = WhitelistPublicKeyMsg {
        public_key: test_pk(),
    };

    let res = whitelist_key(deps.as_mut(), env.clone(), info, msg);

    assert!(res.is_ok());

    let activation_height = res
        .unwrap()
        .attributes
        .into_iter()
        .find(|a| a.key == "activation_height")
        .unwrap()
        .value;

    assert_eq!(activation_height, format!("{}", env.block.height + 1));
}

#[test]
fn checks_invalid_keys() {
    let mut deps = mock_dependencies();
    default_instantiate(deps.as_mut());

    let env = mock_env();
    let info = mock_info("executor", &[coin(1000, "uluna")]);

    let msg = WhitelistPublicKeyMsg {
        public_key: PublicKey::from_bytes(&[0u8; 32]),
    };

    let res = whitelist_key(deps.as_mut(), env, info, msg);

    assert_eq!(res.unwrap_err(), ContractError::InvalidPublicKey {});
}

#[test]
fn rejects_already_whitelisted_keys() {
    let mut deps = mock_dependencies();
    default_instantiate(deps.as_mut());

    let env = mock_env();
    let info = mock_info("executor", &[coin(1000, "uluna")]);

    let msg = WhitelistPublicKeyMsg {
        public_key: test_pk(),
    };

    let res = whitelist_key(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
    assert_eq!(res.attributes.len(), 3);

    let res = whitelist_key(deps.as_mut(), env, info, msg);

    assert_eq!(res.unwrap_err(), ContractError::KeyAlreadyWhitelisted {});
}

#[test]
fn rejects_without_deposit() {
    let mut deps = mock_dependencies();
    default_instantiate(deps.as_mut());

    let env = mock_env();
    let info = mock_info("executor", &[coin(0, "uluna")]);

    let msg = WhitelistPublicKeyMsg {
        public_key: test_pk(),
    };

    let res = whitelist_key(deps.as_mut(), env.clone(), info, msg.clone());
    assert_eq!(res.unwrap_err(), ContractError::InsufficientFunds {});

    let info = mock_info("executor", &[coin(1000, "uatom")]);
    let res = whitelist_key(deps.as_mut(), env, info, msg);
    assert_eq!(res.unwrap_err(), ContractError::InsufficientFunds {});
}

#[test]
fn activates_after_period() {
    let mut deps = mock_dependencies();
    default_instantiate(deps.as_mut());

    let mut env = mock_env();
    let info = mock_info("executor", &[coin(1000, "uluna")]);

    let msg = WhitelistPublicKeyMsg {
        public_key: test_pk(),
    };

    whitelist_key(deps.as_mut(), env.clone(), info, msg).unwrap();

    let status_res = key_status_query(
        deps.as_ref(),
        env.clone(),
        KeyStatusQuery {
            public_key: test_pk(),
        },
    )
    .unwrap();

    assert!(status_res.whitelisted);
    assert!(!status_res.active);
    assert_eq!(status_res.activation_height, env.block.height + 1);

    env.block.height += 1;
    let status_res = key_status_query(
        deps.as_ref(),
        env.clone(),
        KeyStatusQuery {
            public_key: test_pk(),
        },
    )
    .unwrap();

    assert!(status_res.whitelisted);
    assert!(status_res.active);
    assert_eq!(status_res.activation_height, env.block.height);
}
