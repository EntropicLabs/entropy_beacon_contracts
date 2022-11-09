use cosmwasm_std::{
    coin,
    testing::{mock_dependencies, mock_env, mock_info, MockApi, MockQuerier, MockStorage},
    to_binary, Addr, Empty, Env, OwnedDeps, Uint128,
};

use ecvrf_rs::{encode_hex, Proof, SecretKey};
use entropy_beacon_cosmos::{
    beacon::RequestEntropyMsg,
    provide::{SubmitEntropyMsg, WhitelistPublicKeyMsg},
};

use crate::{execute, query, ContractError};

use super::{default_instantiate, test_pk, test_sk};

pub fn request_entropy(
    deps: &mut OwnedDeps<MockStorage, MockApi, MockQuerier, Empty>,
    env: &mut Env,
) {
    let info = mock_info("requester", &[coin(1100, "uluna")]);

    let request_msg = RequestEntropyMsg {
        callback_gas_limit: 1000,
        callback_address: Addr::unchecked("callback_address".to_string()),
        callback_msg: to_binary("callback_msg".as_bytes()).unwrap(),
    };

    execute::request_entropy(deps.as_mut(), env.clone(), info, request_msg).unwrap();
}

pub fn setup_contract(
    deps: &mut OwnedDeps<MockStorage, MockApi, MockQuerier, Empty>,
    env: &mut Env,
) {
    default_instantiate(deps.as_mut());

    let info = mock_info("submitter", &[coin(1000, "uluna")]);

    let msg = WhitelistPublicKeyMsg {
        public_key: test_pk(),
    };
    execute::whitelist_key(deps.as_mut(), env.clone(), info, msg).unwrap();
    env.block.height += 1;

    request_entropy(deps, env);
}

#[test]
fn submits_correctly() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();
    setup_contract(&mut deps, &mut env);

    let info = mock_info("submitter", &[]);
    let last_entropy = "".to_string();
    let proof = Proof::new(&test_sk(), last_entropy).unwrap();

    let msg = SubmitEntropyMsg {
        proof: proof.clone(),
        request_ids: vec![Uint128::zero()],
    };
    let res = execute::submit_entropy(deps.as_mut(), env.clone(), info, msg);
    assert!(res.is_ok());

    let last_entropy = query::last_entropy_query(deps.as_ref()).unwrap().entropy;
    assert_eq!(last_entropy, encode_hex(&proof.verify().unwrap()));
}

#[test]
fn rejects_wrong_message() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();
    setup_contract(&mut deps, &mut env);

    let info = mock_info("submitter", &[]);
    let fake_last_entropy = "NOT LAST ENTROPY".to_string();
    let proof = Proof::new(&test_sk(), fake_last_entropy).unwrap();

    let msg = SubmitEntropyMsg {
        proof,
        request_ids: vec![Uint128::zero()],
    };
    let res = execute::submit_entropy(deps.as_mut(), env.clone(), info, msg);
    assert_eq!(res.unwrap_err(), ContractError::InvalidMessage {});
}

#[test]
fn rejects_inactive_keys() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();
    setup_contract(&mut deps, &mut env);
    env.block.height -= 1;

    let info = mock_info("submitter", &[]);
    let last_entropy = "".to_string();
    let proof = Proof::new(&test_sk(), last_entropy).unwrap();

    let msg = SubmitEntropyMsg {
        proof,
        request_ids: vec![Uint128::zero()],
    };
    let res = execute::submit_entropy(deps.as_mut(), env.clone(), info, msg);
    assert_eq!(
        res.unwrap_err(),
        ContractError::KeyNotActive {
            activation_height: env.block.height + 1
        }
    );
}

#[test]
fn rejects_invalid_keys() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();
    setup_contract(&mut deps, &mut env);

    let info = mock_info("submitter", &[]);
    let last_entropy = "".to_string();
    let sk = SecretKey::from_slice(&[0; 32]);
    let proof = Proof::new(&sk, last_entropy).unwrap();

    let msg = SubmitEntropyMsg {
        proof,
        request_ids: vec![Uint128::zero()],
    };
    let res = execute::submit_entropy(deps.as_mut(), env.clone(), info, msg);
    assert_eq!(res.unwrap_err(), ContractError::KeyNotWhitelisted {});
}

#[test]
fn rejects_unauthorized_sender() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();
    setup_contract(&mut deps, &mut env);

    let info = mock_info("someone else", &[]);
    let last_entropy = "".to_string();
    let proof = Proof::new(&test_sk(), last_entropy).unwrap();

    let msg = SubmitEntropyMsg {
        proof,
        request_ids: vec![Uint128::zero()],
    };
    let res = execute::submit_entropy(deps.as_mut(), env.clone(), info, msg);
    assert_eq!(res.unwrap_err(), ContractError::Unauthorized {});
}

#[test]
fn rejects_invalid_proofs() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();
    setup_contract(&mut deps, &mut env);

    let info = mock_info("submitter", &[]);
    let last_entropy = "".to_string();
    let proof = Proof {
        signer: test_pk(),
        message_bytes: last_entropy.into(),
        proof_bytes: vec![0; 80],
    };

    let msg = SubmitEntropyMsg {
        proof,
        request_ids: vec![Uint128::zero()],
    };
    let res = execute::submit_entropy(deps.as_mut(), env.clone(), info, msg);
    assert_eq!(res.unwrap_err(), ContractError::InvalidProof {});
}

#[test]
fn submit_all_multiple_request_ids() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();
    setup_contract(&mut deps, &mut env);
    request_entropy(&mut deps, &mut env);

    let info = mock_info("submitter", &[]);
    let last_entropy = "".to_string();
    let proof = Proof::new(&test_sk(), last_entropy).unwrap();

    let msg = SubmitEntropyMsg {
        proof,
        request_ids: vec![Uint128::zero(), Uint128::from(1u128)],
    };
    let res = execute::submit_entropy(deps.as_mut(), env.clone(), info, msg);
    assert!(res.is_ok());

    let res = res.unwrap();
    assert_eq!(res.messages.len(), 3);
}

#[test]
fn submit_none_multiple_request_ids() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();
    setup_contract(&mut deps, &mut env);
    request_entropy(&mut deps, &mut env);

    let info = mock_info("submitter", &[]);
    let last_entropy = "".to_string();
    let proof = Proof::new(&test_sk(), last_entropy).unwrap();

    let msg = SubmitEntropyMsg {
        proof,
        request_ids: vec![],
    };
    let res = execute::submit_entropy(deps.as_mut(), env.clone(), info, msg);
    assert!(res.is_ok());

    let res = res.unwrap();
    assert_eq!(res.messages.len(), 3);
}

#[test]
fn submit_one_multiple_request_ids() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();
    setup_contract(&mut deps, &mut env);
    request_entropy(&mut deps, &mut env);

    let info = mock_info("submitter", &[]);
    let last_entropy = "".to_string();
    let proof = Proof::new(&test_sk(), last_entropy).unwrap();

    let msg = SubmitEntropyMsg {
        proof,
        request_ids: vec![Uint128::from(1u128)],
    };
    let res = execute::submit_entropy(deps.as_mut(), env.clone(), info, msg);
    assert!(res.is_ok());

    let res = res.unwrap();
    assert_eq!(res.messages.len(), 2);
}

#[test]
fn errors_on_invalid_request_id() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();
    setup_contract(&mut deps, &mut env);
    request_entropy(&mut deps, &mut env);

    let info = mock_info("submitter", &[]);
    let last_entropy = "".to_string();
    let proof = Proof::new(&test_sk(), last_entropy).unwrap();

    let msg = SubmitEntropyMsg {
        proof,
        request_ids: vec![Uint128::from(2u128)],
    };
    let res = execute::submit_entropy(deps.as_mut(), env.clone(), info, msg);
    assert_eq!(
        res.unwrap_err(),
        ContractError::NoMatchingRequests { request_id: 2u128 }
    );
}
