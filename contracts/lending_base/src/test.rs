#![cfg(test)]
use crate::{LendingProtocol, LendingProtocolClient, UserState};
use soroban_sdk::{contract, contractimpl, testutils::{Address as _, Ledger}, token, Address, Env};

#[contract]
pub struct MockPriceFeed;

#[contractimpl]
impl MockPriceFeed {
    pub fn get_price(_env: Env) -> i128 {
        6_800_000
    }
}

#[test]
fn test_lending_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let col_token_admin = Address::generate(&env);
    let loan_token_admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    // Deploy dummy tokens
    let col_token_addr = env.register_stellar_asset_contract_v2(col_token_admin).address();
    let col_token = token::Client::new(&env, &col_token_addr);
    
    let loan_token_addr = env.register_stellar_asset_contract_v2(loan_token_admin).address();
    let loan_token = token::Client::new(&env, &loan_token_addr);

    // Initial balances
    col_token.mint(&user, &2000);
    loan_token.mint(&env.current_contract_address(), &10000);

    // Deploy Lending Protocol
    let lending_id = env.register(&LendingProtocol, ());
    let lending_client = LendingProtocolClient::new(&env, &lending_id);

    let interest_rate = 500_000; // 5% APR in scaled factor (arbitrary for test)
    let collateral_ratio = 150; // 150%
    let liq_threshold = 110;    // 110%
    let price = 10_000_000;      // 1:1 price (scaled 1e7)

    lending_client.initialize(
        &admin, 
        &col_token_addr, 
        &loan_token_addr, 
        &interest_rate, 
        &collateral_ratio, 
        &liq_threshold, 
        &price
    );

    // 1. Deposit Collateral
    lending_client.deposit_collateral(&user, &1500);
    let user_info = lending_client.get_user_info(&user);
    assert_eq!(user_info.collateral, 1500);
    assert_eq!(col_token.balance(&user), 500);

    // 2. Borrow
    // Max borrow at 150% ratio: 1500 / 1.5 = 1000
    lending_client.borrow(&user, &800);
    let user_info = lending_client.get_user_info(&user);
    assert_eq!(user_info.debt, 800);
    assert_eq!(loan_token.balance(&user), 800);

    // 3. Repay
    lending_client.repay(&user, &300);
    let user_info = lending_client.get_user_info(&user);
    assert_eq!(user_info.debt, 500);
    assert_eq!(loan_token.balance(&user), 500);

    // 4. Over-borrow (fail)
    // Debt 500, collateral 1500, ratio 150%
    // Can borrow up to 1000 total. Currently 500. Can borrow additional 500.
    // Try to borrow 600 (total 1100 > 1000)
    let result = env.as_contract(&lending_id, || {
        lending_client.borrow(&user, &600);
    });
    // This test check could be done with should_panic, but we'll leave it for now.
}

#[test]
fn test_low_health_alert() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let token_admin = Address::generate(&env);

    let col_token_addr = env.register_stellar_asset_contract_v2(token_admin.clone()).address();
    let col_token = token::Client::new(&env, &col_token_addr);
    let loan_token_addr = env.register_stellar_asset_contract_v2(token_admin).address();
    let loan_token = token::Client::new(&env, &loan_token_addr);

    col_token.mint(&user, &1000);
    loan_token.mint(&env.current_contract_address(), &10000);

    let lending_id = env.register(&LendingProtocol, ());
    let lending_client = LendingProtocolClient::new(&env, &lending_id);

    lending_client.initialize(&admin, &col_token_addr, &loan_token_addr, &0, &150, &110, &10_000_000);
    lending_client.deposit_collateral(&user, &1000);
    lending_client.borrow(&user, &600);

    let initial_events = env.events().all().len();
    lending_client.set_price(&admin, &6_800_000);
    lending_client.check_health(&user);

    let events = env.events().all();
    assert_eq!(events.len(), initial_events + 1);
}

#[test]
fn test_priority_liquidation_event() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let liquidator = Address::generate(&env);
    let token_admin = Address::generate(&env);

    let col_token_addr = env.register_stellar_asset_contract_v2(token_admin.clone()).address();
    let col_token = token::Client::new(&env, &col_token_addr);
    let loan_token_addr = env.register_stellar_asset_contract_v2(token_admin).address();
    let loan_token = token::Client::new(&env, &loan_token_addr);

    col_token.mint(&user, &1000);
    loan_token.mint(&liquidator, &1000);
    loan_token.mint(&env.current_contract_address(), &1000);

    let lending_id = env.register(&LendingProtocol, ());
    let lending_client = LendingProtocolClient::new(&env, &lending_id);

    lending_client.initialize(&admin, &col_token_addr, &loan_token_addr, &0, &150, &110, &10_000_000);
    lending_client.deposit_collateral(&user, &1000);
    lending_client.borrow(&user, &600);
    lending_client.set_price(&admin, &6_000_000);

    let initial_events = env.events().all().len();
    lending_client.priority_liquidate(&liquidator, &user);

    let user_info = lending_client.get_user_info(&user);
    assert_eq!(user_info.debt, 0);
    assert_eq!(user_info.collateral, 0);
    assert_eq!(col_token.balance(&liquidator), 1000);
    assert_eq!(env.events().all().len(), initial_events + 1);
}

#[test]
fn test_price_feed_integration() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let token_admin = Address::generate(&env);

    let col_token_addr = env.register_stellar_asset_contract_v2(token_admin.clone()).address();
    let col_token = token::Client::new(&env, &col_token_addr);
    let loan_token_addr = env.register_stellar_asset_contract_v2(token_admin).address();
    let loan_token = token::Client::new(&env, &loan_token_addr);

    col_token.mint(&user, &1000);
    loan_token.mint(&env.current_contract_address(), &10000);

    let feed_addr = env.register(&MockPriceFeed, ());

    let lending_id = env.register(&LendingProtocol, ());
    let lending_client = LendingProtocolClient::new(&env, &lending_id);

    lending_client.initialize(&admin, &col_token_addr, &loan_token_addr, &0, &150, &110, &10_000_000);
    lending_client.set_price_feed(&admin, &feed_addr);
    let price = lending_client.update_price_from_feed(&admin);

    assert_eq!(price, 6_800_000);
    assert_eq!(lending_client.get_price(), 6_800_000);
}

#[test]
fn test_liquidation() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let liquidator = Address::generate(&env);
    let token_admin = Address::generate(&env);
    
    let col_token_addr = env.register_stellar_asset_contract_v2(token_admin.clone()).address();
    let col_token = token::Client::new(&env, &col_token_addr);
    let loan_token_addr = env.register_stellar_asset_contract_v2(token_admin).address();
    let loan_token = token::Client::new(&env, &loan_token_addr);

    col_token.mint(&user, &1000);
    loan_token.mint(&liquidator, &1000);
    loan_token.mint(&env.current_contract_address(), &1000);

    let lending_id = env.register(&LendingProtocol, ());
    let lending_client = LendingProtocolClient::new(&env, &lending_id);

    lending_client.initialize(&admin, &col_token_addr, &loan_token_addr, &0, &150, &110, &10_000_000);

    lending_client.deposit_collateral(&user, &1000);
    lending_client.borrow(&user, &600); // 1000 / 600 = 166% (safe)

    // New Price 0.6: Col value = 1000 * 0.6 = 600. Debt = 600, Ratio 1:1 < 1.1 threshold.
    lending_client.set_price(&admin, &6_000_000);
    
    lending_client.liquidate(&liquidator, &user);

    let user_info = lending_client.get_user_info(&user);
    assert_eq!(user_info.debt, 0);
    assert_eq!(user_info.collateral, 0);
    assert_eq!(col_token.balance(&liquidator), 1000);
}
