#![cfg(test)]
use super::*;
use soroban_sdk::token::{Client as TokenClient, StellarAssetClient};
use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env, Vec};

#[test]
fn test_escrow_release_flow() {
    let env = Env::default();
    env.mock_all_auths();

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);
    let arbitrator = Address::generate(&env);

    let token_addr = env.register_stellar_asset_contract_v2(sender.clone()).address();
    let token_admin = StellarAssetClient::new(&env, &token_addr);
    let token = TokenClient::new(&env, &token_addr);

    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    token_admin.mint(&sender, &1000);
    assert_eq!(token.balance(&sender), 1000);

    let amount = 500;
    let unlock_time = 1000;
    let escrow_id = client.initiate_escrow(&sender, &recipient, &arbitrator, &token_addr, &amount, &unlock_time);

    assert_eq!(token.balance(&sender), 500);
    assert_eq!(token.balance(&contract_id), 500);

    client.release_funds(&escrow_id, &arbitrator);

    assert_eq!(token.balance(&recipient), 500);
    assert_eq!(token.balance(&contract_id), 0);
}

#[test]
fn test_escrow_refund_flow() {
    let env = Env::default();
    env.mock_all_auths();

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);
    let arbitrator = Address::generate(&env);
    let token_addr = env.register_stellar_asset_contract_v2(sender.clone()).address();
    let token_admin = StellarAssetClient::new(&env, &token_addr);
    let token = TokenClient::new(&env, &token_addr);

    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    token_admin.mint(&sender, &1000);

    let unlock_time = 1000;
    let escrow_id = client.initiate_escrow(&sender, &recipient, &arbitrator, &token_addr, &500, &unlock_time);

    env.ledger().set_timestamp(1500);
    client.cancel_escrow(&escrow_id, &sender);

    assert_eq!(token.balance(&sender), 1000);
    assert_eq!(token.balance(&contract_id), 0);
}

#[test]
fn test_auto_resolve_after_timeout() {
    let env = Env::default();
    env.mock_all_auths();

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);
    let arbitrator = Address::generate(&env);
    let token_addr = env.register_stellar_asset_contract_v2(sender.clone()).address();
    let token_admin = StellarAssetClient::new(&env, &token_addr);
    let token = TokenClient::new(&env, &token_addr);

    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    token_admin.mint(&sender, &1000);

    let unlock_time = 1000;
    let escrow_id = client.initiate_escrow(&sender, &recipient, &arbitrator, &token_addr, &300, &unlock_time);

    env.ledger().set_timestamp(1501);
    client.auto_resolve(&escrow_id);

    assert_eq!(token.balance(&sender), 1000);
    assert_eq!(token.balance(&contract_id), 0);
}

#[test]
#[should_panic(expected = "Not authorized to release")]
fn test_unauthorized_release() {
    let env = Env::default();
    env.mock_all_auths();

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);
    let arbitrator = Address::generate(&env);
    let token_addr = env.register_stellar_asset_contract_v2(sender.clone()).address();
    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    StellarAssetClient::new(&env, &token_addr).mint(&sender, &1000);

    let escrow_id = client.initiate_escrow(&sender, &recipient, &arbitrator, &token_addr, &500, &1000);

    let attacker = Address::generate(&env);
    client.release_funds(&escrow_id, &attacker);
}

#[test]
fn test_arbitrator_can_cancel() {
    let env = Env::default();
    env.mock_all_auths();

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);
    let arbitrator = Address::generate(&env);
    let token_addr = env.register_stellar_asset_contract_v2(sender.clone()).address();
    let token_admin = StellarAssetClient::new(&env, &token_addr);
    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    token_admin.mint(&sender, &1000);
    let escrow_id = client.initiate_escrow(&sender, &recipient, &arbitrator, &token_addr, &500, &1000);

    client.cancel_escrow(&escrow_id, &arbitrator);
    assert_eq!(token.balance(&sender), 500);
    assert_eq!(token.balance(&contract_id), 0);
}

#[test]
fn test_escrow_dispute_and_resolve_with_fees() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let platform = Address::generate(&env);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);
    let arbitrator = Address::generate(&env);

    let token_addr = env.register_stellar_asset_contract_v2(admin.clone()).address();
    let token_admin = StellarAssetClient::new(&env, &token_addr);
    let token = TokenClient::new(&env, &token_addr);

    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    client.initialize(&admin, &platform, &200, &100);
    token_admin.mint(&sender, &1000);

    let escrow_id = client.initiate_escrow(&sender, &recipient, &arbitrator, &token_addr, &1000, &1000);
    client.dispute_escrow(&escrow_id, &recipient, &Vec::from_slice(&env, b"evidence_hash"));
    client.resolve_dispute(&escrow_id, &arbitrator, &true, &Vec::from_slice(&env, b"arb_hash"));

    assert_eq!(token.balance(&recipient), 970);
    assert_eq!(token.balance(&arbitrator), 10);
    assert_eq!(token.balance(&platform), 20);
    assert_eq!(token.balance(&contract_id), 0);
}
