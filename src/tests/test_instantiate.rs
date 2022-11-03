use cosmwasm_std::{
    testing::{mock_dependencies, mock_env, mock_info},
    Addr, Attribute, Decimal, Uint128,
};
use entropy_beacon_cosmos::provide::{BeaconConfigResponse, KeyStatusQuery};

use crate::{
    msg::InstantiateMsg,
    tests::default_instantiate, query, contract::instantiate,
};

use super::test_pk;

#[test]
fn instantiates_correctly() {
    let mut deps = mock_dependencies();
    let res = default_instantiate(deps.as_mut());
    assert_eq!(2, res.attributes.len());
    assert_eq!(
        Attribute {
            key: "action".to_string(),
            value: "instantiate".to_string()
        },
        res.attributes.get(0).unwrap()
    );
    assert_eq!(
        Attribute {
            key: "owner".to_string(),
            value: "creator".to_string()
        },
        res.attributes.get(1).unwrap()
    );

    let res = query::beacon_config_query(deps.as_ref()).unwrap();
    assert_eq!(
        res,
        BeaconConfigResponse {
            whitelist_deposit_amt: Uint128::from(1000u128),
            protocol_fee: 100,
            submitter_share: Decimal::percent(80),
            key_activation_delay: 1,
        }
    );
}

#[test]
fn with_prewhitelisted_keys() {
    let mut deps = mock_dependencies();
    let msg = InstantiateMsg {
        whitelist_deposit_amt: Uint128::from(1000u128),
        refund_increment_amt: Uint128::from(1000u128),
        key_activation_delay: 1,
        protocol_fee: 100,
        submitter_share: 80,
        native_denom: "uluna".to_string(),
        whitelisted_keys: vec![(Addr::unchecked("creator"), test_pk())],
        belief_gas_price: Decimal::percent(15),
        permissioned: false,
        test_mode: false,
    };
    let env = mock_env();
    let info = mock_info("creator", vec![].as_slice());

    // we can just call .unwrap() to assert this was a success
    instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    let status_res = query::key_status_query(
        deps.as_ref(),
        env.clone(),
        KeyStatusQuery {
            public_key: test_pk(),
        },
    )
    .unwrap();
    assert!(status_res.whitelisted);
    assert_eq!(status_res.activation_height, env.block.height + 1);
}
