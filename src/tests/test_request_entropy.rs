use cosmwasm_std::{
    coin,
    testing::{mock_dependencies, mock_env, mock_info, MockApi, MockQuerier, MockStorage},
    to_binary, Addr, Empty, Env, OwnedDeps, Uint128,
};

use entropy_beacon_cosmos::{
    beacon::RequestEntropyMsg,
    provide::{ActiveRequestInfo, ActiveRequestsQuery, WhitelistPublicKeyMsg},
};

use crate::{execute, query, ContractError};

use super::{default_instantiate, test_pk};

fn setup_contract(deps: &mut OwnedDeps<MockStorage, MockApi, MockQuerier, Empty>, env: &mut Env) {
    default_instantiate(deps.as_mut());

    let info = mock_info("submitter", &[coin(1000, "uluna")]);

    let msg = WhitelistPublicKeyMsg {
        public_key: test_pk(),
    };
    execute::whitelist_key(deps.as_mut(), env.clone(), info, msg).unwrap();
    env.block.height += 1;
}

#[test]
fn requests_correctly() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();
    setup_contract(&mut deps, &mut env);

    let info = mock_info("requester", &[coin(1100, "uluna")]);

    let request_msg = RequestEntropyMsg {
        callback_gas_limit: 1000,
        callback_address: Addr::unchecked("callback_address".to_string()),
        callback_msg: to_binary("callback_msg".as_bytes()).unwrap(),
    };

    let res = execute::request_entropy(deps.as_mut(), env.clone(), info, request_msg);
    assert!(res.is_ok());

    let active_query_msg = ActiveRequestsQuery {
        start_after: None,
        limit: None,
    };
    let active_query_res = query::active_requests_query(deps.as_ref(), active_query_msg).unwrap();
    assert_eq!(active_query_res.requests.len(), 1);
    assert_eq!(
        active_query_res.requests[0],
        ActiveRequestInfo {
            id: 0,
            callback_gas_limit: 1000,
            callback_address: Addr::unchecked("callback_address".to_string()),
            submitter: Addr::unchecked("requester".to_string()),
            submitted_block_height: env.block.height,
            submitted_bounty_amount: Uint128::from(1100u128),
        }
    );
}

#[test]
fn rejects_insufficient_funds() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();
    setup_contract(&mut deps, &mut env);

    let info = mock_info("requester", &[coin(150u128, "uluna")]);

    let request_msg = RequestEntropyMsg {
        callback_gas_limit: 1000,
        callback_address: Addr::unchecked("callback_address".to_string()),
        callback_msg: to_binary("callback_msg".as_bytes()).unwrap(),
    };

    let res = execute::request_entropy(deps.as_mut(), env.clone(), info, request_msg);
    assert_eq!(res.unwrap_err(), ContractError::InsufficientFunds {});
}
