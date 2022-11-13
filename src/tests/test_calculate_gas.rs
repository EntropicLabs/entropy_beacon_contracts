use cosmwasm_std::{
    coin,
    testing::{mock_dependencies, mock_env, mock_info},
    Decimal, Uint128,
};
use entropy_beacon_cosmos::{beacon::CalculateFeeQuery, msg::InstantiateMsg};

use crate::{execute, query, tests::default_instantiate, ContractError, contract::instantiate};

#[test]
fn calculates_correctly() {
    let mut deps = mock_dependencies();
    default_instantiate(deps.as_mut());

    let msg = CalculateFeeQuery {
        callback_gas_limit: 1000,
    };

    let res = query::calculate_fee_query(deps.as_ref(), msg);
    assert!(res.is_ok());

    let res = res.unwrap();
    assert_eq!(res.fee, 250); // 1000 * 0.15 + 100
    assert_eq!(res.gas_price, Decimal::percent(15));
}

#[test]
fn updates_belief_price() {
    let mut deps = mock_dependencies();
    default_instantiate(deps.as_mut());

    let env = mock_env();
    let info = mock_info("creator", &[coin(1000, "uluna")]);

    let msg = CalculateFeeQuery {
        callback_gas_limit: 1000,
    };

    let res = query::calculate_fee_query(deps.as_ref(), msg.clone());
    assert!(res.is_ok());

    let res = res.unwrap();
    assert_eq!(res.fee, 250);
    assert_eq!(res.gas_price, Decimal::percent(15));

    let res = execute::update_gas_price(deps.as_mut(), env, info, Decimal::percent(500));
    assert!(res.is_ok());

    let res = query::calculate_fee_query(deps.as_ref(), msg);
    assert!(res.is_ok());

    let res = res.unwrap();
    assert_eq!(res.fee, 5100);
    assert_eq!(res.gas_price, Decimal::percent(500));
}

#[test]
fn rejects_unauthorized() {
    let mut deps = mock_dependencies();
    default_instantiate(deps.as_mut());

    let env = mock_env();
    let info = mock_info("anyone", &[coin(1000, "uluna")]);

    let res = execute::update_gas_price(deps.as_mut(), env, info, Decimal::percent(500));
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), ContractError::Unauthorized {});
}


#[test]
fn subsidization_works() {
    let mut deps = mock_dependencies();
    let msg = InstantiateMsg {
        whitelist_deposit_amt: Uint128::from(1000u128),
        refund_increment_amt: Uint128::from(1000u128),
        key_activation_delay: 1,
        protocol_fee: 100,
        submitter_share: 80,
        native_denom: "uluna".to_string(),
        whitelisted_keys: vec![],
        belief_gas_price: Decimal::percent(15),
        permissioned: false,
        test_mode: false,
        subsidize_callbacks: true,
    };
    let env = mock_env();
    let info = mock_info("creator", vec![].as_slice());

    instantiate(deps.as_mut(), env, info, msg).unwrap();

    let msg = CalculateFeeQuery {
        callback_gas_limit: 1000,
    };

    let res = query::calculate_fee_query(deps.as_ref(), msg);
    assert!(res.is_ok());

    let res = res.unwrap();
    assert_eq!(res.fee, 100); // 1000 * 0.15 * 0 + 100
    assert_eq!(res.gas_price, Decimal::percent(15));
}