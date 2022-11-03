use cosmwasm_std::{
    coin,
    testing::{mock_dependencies, mock_env, mock_info},
};
use ecvrf_rs::Proof;
use entropy_beacon_cosmos::provide::{ActiveRequestsQuery, SubmitEntropyMsg};

use crate::{
    execute,
    query::active_requests_query,
    tests::{
        test_sk,
        test_submit_entropy::{request_entropy, setup_contract},
    },
};

#[test]
fn pagination_works() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();
    setup_contract(&mut deps, &mut env);

    for _ in 0..29 {
        request_entropy(&mut deps, &mut env);
    }

    let active_query_msg = ActiveRequestsQuery {
        start_after: None,
        limit: None, // Default 10
    };
    let response = active_requests_query(deps.as_ref(), active_query_msg);
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!(response.requests.len(), 10);
    assert_eq!(
        response.requests.iter().map(|r| r.id).collect::<Vec<_>>(),
        (0..10).collect::<Vec<_>>()
    );

    let active_query_msg = ActiveRequestsQuery {
        start_after: Some(9),
        limit: None, // Default 10
    };

    let response = active_requests_query(deps.as_ref(), active_query_msg);
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!(response.requests.len(), 10);

    assert_eq!(
        response.requests.iter().map(|r| r.id).collect::<Vec<_>>(),
        (10..20).collect::<Vec<_>>()
    );

    let active_query_msg = ActiveRequestsQuery {
        start_after: Some(19),
        limit: None, // Default 10
    };

    let response = active_requests_query(deps.as_ref(), active_query_msg);
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!(response.requests.len(), 10);

    assert_eq!(
        response.requests.iter().map(|r| r.id).collect::<Vec<_>>(),
        (20..30).collect::<Vec<_>>()
    );
}

#[test]
fn pagination_works_when_exceeding() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();
    setup_contract(&mut deps, &mut env);

    for _ in 0..9 {
        request_entropy(&mut deps, &mut env);
    }

    let active_query_msg = ActiveRequestsQuery {
        start_after: None,
        limit: Some(30),
    };
    let response = active_requests_query(deps.as_ref(), active_query_msg);
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!(response.requests.len(), 10);
    assert_eq!(
        response.requests.iter().map(|r| r.id).collect::<Vec<_>>(),
        (0..10).collect::<Vec<_>>()
    );

    let active_query_msg = ActiveRequestsQuery {
        start_after: Some(9),
        limit: None, // Default 10
    };

    let response = active_requests_query(deps.as_ref(), active_query_msg);
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!(response.requests.len(), 0);

    let active_query_msg = ActiveRequestsQuery {
        start_after: Some(999),
        limit: None, // Default 10
    };

    let response = active_requests_query(deps.as_ref(), active_query_msg);
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!(response.requests.len(), 0);
}

#[test]
fn max_pagination_limit_obeyed() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();
    setup_contract(&mut deps, &mut env);

    for _ in 0..100 {
        request_entropy(&mut deps, &mut env);
    }

    let active_query_msg = ActiveRequestsQuery {
        start_after: None,
        limit: Some(100),
    };
    let response = active_requests_query(deps.as_ref(), active_query_msg);
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!(response.requests.len(), 30);
    assert_eq!(
        response.requests.iter().map(|r| r.id).collect::<Vec<_>>(),
        (0..30).collect::<Vec<_>>()
    );
}

#[test]
fn works_with_removed_elements() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();
    setup_contract(&mut deps, &mut env);

    for _ in 0..100 {
        request_entropy(&mut deps, &mut env);
    }

    let info = mock_info("submitter", &[coin(1000, "uluna")]);
    let last_entropy = "".to_string();
    let proof = Proof::new(&test_sk(), last_entropy).unwrap();

    let msg = SubmitEntropyMsg {
        proof,
        request_ids: (0..30).collect(),
    };
    let res = execute::submit_entropy(deps.as_mut(), env.clone(), info, msg);
    assert!(res.is_ok());

    let active_query_msg = ActiveRequestsQuery {
        start_after: None,
        limit: None,
    };
    let response = active_requests_query(deps.as_ref(), active_query_msg);
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!(response.requests.len(), 10);
    assert_eq!(
        response.requests.iter().map(|r| r.id).collect::<Vec<_>>(),
        (30..40).collect::<Vec<_>>()
    );

    let active_query_msg = ActiveRequestsQuery {
        start_after: Some(10),
        limit: None,
    };
    let response = active_requests_query(deps.as_ref(), active_query_msg);
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!(response.requests.len(), 10);
    assert_eq!(
        response.requests.iter().map(|r| r.id).collect::<Vec<_>>(),
        (30..40).collect::<Vec<_>>()
    );

    let active_query_msg = ActiveRequestsQuery {
        start_after: Some(39),
        limit: None,
    };
    let response = active_requests_query(deps.as_ref(), active_query_msg);
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!(response.requests.len(), 10);
    assert_eq!(
        response.requests.iter().map(|r| r.id).collect::<Vec<_>>(),
        (40..50).collect::<Vec<_>>()
    );
}
