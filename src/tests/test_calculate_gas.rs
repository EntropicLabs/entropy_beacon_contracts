use cosmwasm_std::{
    coin,
    testing::{mock_dependencies, mock_env, mock_info},
    Decimal,
};
use entropy_beacon_cosmos::beacon::CalculateFeeQuery;

use crate::{execute, query, tests::default_instantiate, ContractError};

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
