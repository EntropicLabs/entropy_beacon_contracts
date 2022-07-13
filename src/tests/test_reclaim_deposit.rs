use cosmwasm_std::{
    coin, coins,
    testing::{mock_dependencies, mock_env, mock_info, MockApi, MockQuerier, MockStorage},
    BankMsg, CosmosMsg, Empty, Env, OwnedDeps,
};
use ecvrf::PublicKey;
use entropy_beacon_cosmos::provide::{KeyStatusQuery, ReclaimDepositMsg, WhitelistPublicKeyMsg};

use crate::{
    contract::{key_status_query, reclaim_deposit, whitelist_key},
    state::WHITELISTED_KEYS,
    ContractError,
};

use super::{default_instantiate, test_pk};

fn setup_contract(deps: &mut OwnedDeps<MockStorage, MockApi, MockQuerier, Empty>, env: Env) {
    default_instantiate(deps.as_mut());

    let info = mock_info("executor", &[coin(1000, "uluna")]);

    let msg = WhitelistPublicKeyMsg {
        public_key: test_pk(),
    };
    whitelist_key(deps.as_mut(), env, info, msg).unwrap();
}

#[test]
fn unwhitelists_key() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    setup_contract(&mut deps, env.clone());

    let info = mock_info("executor", &[]);
    let msg = ReclaimDepositMsg {
        public_key: test_pk(),
    };

    let res = reclaim_deposit(deps.as_mut(), env.clone(), info, msg);

    assert!(res.is_ok());

    assert!(WHITELISTED_KEYS
        .load(deps.as_mut().storage, test_pk().as_bytes())
        .is_err());

    let res = key_status_query(
        deps.as_ref(),
        env,
        KeyStatusQuery {
            public_key: test_pk(),
        },
    )
    .unwrap();

    assert!(!res.whitelisted)
}

#[test]
fn returns_deposit() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    setup_contract(&mut deps, env.clone());

    let info = mock_info("executor", &[]);
    let msg = ReclaimDepositMsg {
        public_key: test_pk(),
    };

    let res = reclaim_deposit(deps.as_mut(), env, info, msg);
    assert!(res.is_ok());
    let res = res.unwrap();

    let refund = res
        .attributes
        .into_iter()
        .find(|a| a.key == "refund")
        .unwrap()
        .value;
    assert_eq!(refund, format!("{}", 1000));

    assert_eq!(
        res.messages[0].msg,
        CosmosMsg::Bank(BankMsg::Send {
            to_address: "executor".to_string(),
            amount: coins(1000, "uluna"),
        })
    );
}

#[test]
fn rejects_unwhitelisted_keys() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    setup_contract(&mut deps, env.clone());

    let info = mock_info("executor", &[]);
    let msg = ReclaimDepositMsg {
        public_key: PublicKey::from_bytes(&[0u8; 32]),
    };

    let res = reclaim_deposit(deps.as_mut(), env, info, msg);
    assert_eq!(res.unwrap_err(), ContractError::KeyNotWhitelisted {});
}

#[test]
fn rejects_unauthorized_claimers() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    setup_contract(&mut deps, env.clone());

    let info = mock_info("not_executor", &[]);
    let msg = ReclaimDepositMsg {
        public_key: test_pk(),
    };

    let res = reclaim_deposit(deps.as_mut(), env, info, msg);
    assert_eq!(res.unwrap_err(), ContractError::Unauthorized {});
}
