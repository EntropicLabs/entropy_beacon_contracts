use cosmwasm_std::{
    from_binary,
    testing::{mock_dependencies, mock_env},
    Attribute, Decimal, Uint128,
};
use entropy_beacon_cosmos::{
    msg::QueryMsg,
    provide::{BeaconConfigQuery, BeaconConfigResponse},
};

use crate::{contract::query, tests::default_instantiate};
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

    let config_query_msg = QueryMsg::BeaconConfig(BeaconConfigQuery {});
    let res = query(deps.as_ref(), mock_env(), config_query_msg).unwrap();
    let res = from_binary::<BeaconConfigResponse>(&res).unwrap();
    assert_eq!(
        res,
        BeaconConfigResponse {
            whitelist_deposit_amt: Uint128::from(1000u128),
            protocol_fee: 2,
            submitter_share: Decimal::percent(80),
            key_activation_delay: 1,
        }
    );
}
