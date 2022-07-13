use cosmwasm_std::{
    coin,
    testing::{mock_dependencies, mock_env, mock_info, MockApi, MockQuerier, MockStorage},
    Empty, Env, OwnedDeps,
};

use ecvrf::{encode_hex, Proof, SecretKey};
use entropy_beacon_cosmos::provide::{SubmitEntropyMsg, WhitelistPublicKeyMsg};

use crate::{
    contract::{last_entropy_query, submit_entropy, whitelist_key},
    ContractError,
};

use super::{default_instantiate, test_pk, test_sk};

fn setup_contract(deps: &mut OwnedDeps<MockStorage, MockApi, MockQuerier, Empty>, env: &mut Env) {
    default_instantiate(deps.as_mut());

    let info = mock_info("executor", &[coin(1000, "uluna")]);

    let msg = WhitelistPublicKeyMsg {
        public_key: test_pk(),
    };
    whitelist_key(deps.as_mut(), env.clone(), info, msg).unwrap();
    env.block.height += 1;
}

#[test]
fn submits_correctly() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();
    setup_contract(&mut deps, &mut env);

    let info = mock_info("executor", &[]);
    let last_entropy = "".to_string();
    let proof = Proof::new(&test_sk(), last_entropy).unwrap();

    let msg = SubmitEntropyMsg {
        proof: proof.clone(),
    };
    let res = submit_entropy(deps.as_mut(), env.clone(), info, msg);
    assert!(res.is_ok());

    let last_entropy = last_entropy_query(deps.as_ref()).unwrap().entropy;
    assert_eq!(last_entropy, encode_hex(&proof.verify().unwrap()));
}

#[test]
fn rejects_wrong_message() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();
    setup_contract(&mut deps, &mut env);

    let info = mock_info("executor", &[]);
    let fake_last_entropy = "NOT LAST ENTROPY".to_string();
    let proof = Proof::new(&test_sk(), fake_last_entropy).unwrap();

    let msg = SubmitEntropyMsg { proof };
    let res = submit_entropy(deps.as_mut(), env.clone(), info, msg);
    assert_eq!(res.unwrap_err(), ContractError::InvalidMessage {});
}

#[test]
fn rejects_inactive_keys() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();
    setup_contract(&mut deps, &mut env);
    env.block.height -= 1;

    let info = mock_info("executor", &[]);
    let last_entropy = "".to_string();
    let proof = Proof::new(&test_sk(), last_entropy).unwrap();

    let msg = SubmitEntropyMsg { proof };
    let res = submit_entropy(deps.as_mut(), env.clone(), info, msg);
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

    let info = mock_info("executor", &[]);
    let last_entropy = "".to_string();
    let sk = SecretKey::from_slice(&[0; 32]);
    let proof = Proof::new(&sk, last_entropy).unwrap();

    let msg = SubmitEntropyMsg { proof };
    let res = submit_entropy(deps.as_mut(), env.clone(), info, msg);
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

    let msg = SubmitEntropyMsg { proof };
    let res = submit_entropy(deps.as_mut(), env.clone(), info, msg);
    assert_eq!(res.unwrap_err(), ContractError::Unauthorized {});
}

#[test]
fn rejects_invalid_proofs() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();
    setup_contract(&mut deps, &mut env);

    let info = mock_info("executor", &[]);
    let last_entropy = "".to_string();
    let proof = Proof{
        signer: test_pk(),
        message_bytes: last_entropy.into(),
        proof_bytes: vec![0; 80],
    };

    let msg = SubmitEntropyMsg { proof };
    let res = submit_entropy(deps.as_mut(), env.clone(), info, msg);
    assert_eq!(res.unwrap_err(), ContractError::InvalidProof {});
}
