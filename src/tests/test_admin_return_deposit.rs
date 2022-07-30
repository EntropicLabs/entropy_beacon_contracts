use cosmwasm_std::{
    coin, coins,
    testing::{mock_dependencies, mock_env, mock_info, MockApi, MockQuerier, MockStorage},
    BankMsg, CosmosMsg, Empty, Env, OwnedDeps,
};
use entropy_beacon_cosmos::provide::{
    AdminReturnDepositMsg, KeyStatusQuery, WhitelistPublicKeyMsg,
};

use crate::{
    contract::{admin_return_deposit, key_status_query, whitelist_key},
    state::WHITELISTED_KEYS,
    ContractError,
};

use super::{default_instantiate, test_pk};

fn setup_contract(deps: &mut OwnedDeps<MockStorage, MockApi, MockQuerier, Empty>, env: Env) {
    default_instantiate(deps.as_mut());

    let info = mock_info("submitter", &[coin(1000, "uluna")]);

    let msg = WhitelistPublicKeyMsg {
        public_key: test_pk(),
    };
    whitelist_key(deps.as_mut(), env, info, msg).unwrap();
}

#[test]
fn unwhitelists_and_returns_deposit() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    setup_contract(&mut deps, env.clone());

    let info = mock_info("creator", &[]);
    let msg = AdminReturnDepositMsg { key: test_pk() };

    let res = admin_return_deposit(deps.as_mut(), env.clone(), info, msg);

    assert!(res.is_ok());

    assert!(WHITELISTED_KEYS
        .load(deps.as_mut().storage, test_pk().as_bytes())
        .is_err());

    let res = res.unwrap();
    assert_eq!(
        res.messages[0].msg,
        CosmosMsg::Bank(BankMsg::Send {
            to_address: "submitter".to_string(),
            amount: coins(1000, "uluna"),
        })
    );

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
fn rejects_unauthorized() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    setup_contract(&mut deps, env.clone());

    let info = mock_info("not_creator", &[]);
    let msg = AdminReturnDepositMsg { key: test_pk() };

    let res = admin_return_deposit(deps.as_mut(), env, info, msg);

    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), ContractError::Unauthorized {});
}
