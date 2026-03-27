#![cfg(test)]
use super::*;
use soroban_sdk::token::{Client as TokenClient, StellarAssetClient};
use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env};

#[test]
fn test_escrow_release_flow() {
    let env = Env::default();
    env.mock_all_auths();

    // 1. Setup participants
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);
    let arbitrator = Address::generate(&env);
    
    // 2. Register mock token
    let token_addr = env.register_stellar_asset_contract_v2(sender.clone()).address();
    let token_admin = StellarAssetClient::new(&env, &token_addr);
    let token = TokenClient::new(&env, &token_addr);

    // 3. Register escrow contract
    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    // 4. Initial funding
    token_admin.mint(&sender, &1000);
    assert_eq!(token.balance(&sender), 1000);

    // 5. Initiate Escrow
    let amount = 500;
    let unlock_time = 1000;
    let escrow_id = client.initiate_escrow(&sender, &recipient, &arbitrator, &token_addr, &amount, &unlock_time);

    assert_eq!(token.balance(&sender), 500);
    assert_eq!(token.balance(&contract_id), 500);

    // 6. Arbitrator releases funds
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

    // 1. Attempt refund before unlock time (failure expected)
    // Actually, I'll test the success case first.
    env.ledger().set_timestamp(1500); // After unlock
    client.cancel_escrow(&escrow_id, &sender);

    assert_eq!(token.balance(&sender), 1000);
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

    // Give funds for initiation
    StellarAssetClient::new(&env, &token_addr).mint(&sender, &1000);

    let escrow_id = client.initiate_escrow(&sender, &recipient, &arbitrator, &token_addr, &500, &1000);

    // Attacker tries to release
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

    // Arbitrator cancels even before unlock time
    client.cancel_escrow(&escrow_id, &arbitrator);
}
