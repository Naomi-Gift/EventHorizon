#![cfg(test)]
use super::*;
use soroban_sdk::{
    testutils::Address as _,
    token::{Client as TokenClient, StellarAssetClient},
    Address, Bytes, Env, Vec,
};

fn make_signers(env: &Env, n: u32) -> Vec<Address> {
    let mut v = Vec::new(env);
    for _ in 0..n { v.push_back(Address::generate(env)); }
    v
}

fn setup_2of3(env: &Env) -> (Address, Vec<Address>) {
    let signers = make_signers(env, 3);
    let contract_id = env.register(MultisigWallet, ());
    let client = MultisigWalletClient::new(env, &contract_id);
    client.initialize(&signers, &2);
    (contract_id, signers)
}

#[test]
fn test_token_transfer_2of3() {
    let env = Env::default();
    env.mock_all_auths();

    let (contract_id, signers) = setup_2of3(&env);
    let client = MultisigWalletClient::new(&env, &contract_id);

    // Fund the multisig wallet with a token
    let admin = Address::generate(&env);
    let token_addr = env.register_stellar_asset_contract_v2(admin.clone()).address();
    StellarAssetClient::new(&env, &token_addr).mint(&contract_id, &1000);

    let recipient = Address::generate(&env);
    let empty = Bytes::new(&env);

    // Signer 0 proposes then approves (1/2)
    let tx_id = client.propose(&signers.get(0).unwrap(), &recipient, &empty, &500i128, &token_addr);
    assert_eq!(client.get_tx(&tx_id).status, TxStatus::Pending);
    client.approve(&signers.get(0).unwrap(), &tx_id);
    assert_eq!(client.get_tx(&tx_id).status, TxStatus::Pending);

    // Signer 1 approves → threshold reached → auto-executes (2/2)
    client.approve(&signers.get(1).unwrap(), &tx_id);
    assert_eq!(client.get_tx(&tx_id).status, TxStatus::Executed);
    assert_eq!(TokenClient::new(&env, &token_addr).balance(&recipient), 500);
}

#[test]
fn test_cancel_tx() {
    let env = Env::default();
    env.mock_all_auths();

    let (contract_id, signers) = setup_2of3(&env);
    let client = MultisigWalletClient::new(&env, &contract_id);

    let dummy = Address::generate(&env);
    let empty = Bytes::new(&env);
    let tx_id = client.propose(&signers.get(0).unwrap(), &dummy, &empty, &0i128, &dummy);

    client.cancel(&signers.get(2).unwrap(), &tx_id);
    assert_eq!(client.get_tx(&tx_id).status, TxStatus::Cancelled);
}

#[test]
#[should_panic(expected = "Tx not pending")]
fn test_approve_cancelled_tx_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let (contract_id, signers) = setup_2of3(&env);
    let client = MultisigWalletClient::new(&env, &contract_id);

    let dummy = Address::generate(&env);
    let empty = Bytes::new(&env);
    let tx_id = client.propose(&signers.get(0).unwrap(), &dummy, &empty, &0i128, &dummy);
    client.cancel(&signers.get(0).unwrap(), &tx_id);
    client.approve(&signers.get(1).unwrap(), &tx_id); // should panic
}

#[test]
#[should_panic(expected = "Already approved")]
fn test_double_approve_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let (contract_id, signers) = setup_2of3(&env);
    let client = MultisigWalletClient::new(&env, &contract_id);

    let dummy = Address::generate(&env);
    let empty = Bytes::new(&env);
    let tx_id = client.propose(&signers.get(0).unwrap(), &dummy, &empty, &0i128, &dummy);
    client.approve(&signers.get(1).unwrap(), &tx_id);
    client.approve(&signers.get(1).unwrap(), &tx_id); // double approve
}

#[test]
#[should_panic(expected = "Not a signer")]
fn test_non_signer_cannot_propose() {
    let env = Env::default();
    env.mock_all_auths();

    let (contract_id, _) = setup_2of3(&env);
    let client = MultisigWalletClient::new(&env, &contract_id);

    let outsider = Address::generate(&env);
    let dummy = Address::generate(&env);
    let empty = Bytes::new(&env);
    client.propose(&outsider, &dummy, &empty, &0i128, &dummy);
}

#[test]
fn test_add_remove_signer_via_multisig() {
    let env = Env::default();
    env.mock_all_auths();

    let (contract_id, signers) = setup_2of3(&env);
    let client = MultisigWalletClient::new(&env, &contract_id);

    let new_signer = Address::generate(&env);

    // Propose add_signer call targeting the contract itself
    let empty = Bytes::new(&env);
    let tx_id = client.propose(&signers.get(0).unwrap(), &contract_id, &empty, &0i128, &contract_id);

    // Reach threshold (2-of-3): signer 0 + signer 1 approve
    client.approve(&signers.get(0).unwrap(), &tx_id);
    client.approve(&signers.get(1).unwrap(), &tx_id);
    assert_eq!(client.get_tx(&tx_id).status, TxStatus::Executed);

    // Directly call add_signer (simulating the contract calling itself after approval)
    client.add_signer(&new_signer);
    assert!(client.is_signer(&new_signer));
    assert_eq!(client.get_signers().len(), 4);

    // Remove the new signer
    client.remove_signer(&new_signer);
    assert!(!client.is_signer(&new_signer));
    assert_eq!(client.get_signers().len(), 3);
}

#[test]
#[should_panic(expected = "Already initialized")]
fn test_double_init_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let signers = make_signers(&env, 2);
    let contract_id = env.register(MultisigWallet, ());
    let client = MultisigWalletClient::new(&env, &contract_id);
    client.initialize(&signers, &1);
    client.initialize(&signers, &1);
}

#[test]
#[should_panic(expected = "Threshold exceeds signer count")]
fn test_threshold_exceeds_signers_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let signers = make_signers(&env, 2);
    let contract_id = env.register(MultisigWallet, ());
    let client = MultisigWalletClient::new(&env, &contract_id);
    client.initialize(&signers, &3); // 3 > 2
}

#[test]
fn test_1of1_executes_immediately() {
    let env = Env::default();
    env.mock_all_auths();

    let signers = make_signers(&env, 1);
    let contract_id = env.register(MultisigWallet, ());
    let client = MultisigWalletClient::new(&env, &contract_id);
    client.initialize(&signers, &1);

    let admin = Address::generate(&env);
    let token_addr = env.register_stellar_asset_contract_v2(admin.clone()).address();
    StellarAssetClient::new(&env, &token_addr).mint(&contract_id, &200);

    let recipient = Address::generate(&env);
    let empty = Bytes::new(&env);
    let tx_id = client.propose(&signers.get(0).unwrap(), &recipient, &empty, &200i128, &token_addr);
    client.approve(&signers.get(0).unwrap(), &tx_id);

    assert_eq!(client.get_tx(&tx_id).status, TxStatus::Executed);
    assert_eq!(TokenClient::new(&env, &token_addr).balance(&recipient), 200);
}
