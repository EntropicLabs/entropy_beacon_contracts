use cosmwasm_std::{testing::mock_dependencies, Attribute, Decimal, Uint128};
use entropy_beacon_cosmos::provide::BeaconConfigResponse;

use crate::{contract::beacon_config_query, tests::default_instantiate};
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

    let res = beacon_config_query(deps.as_ref()).unwrap();
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
