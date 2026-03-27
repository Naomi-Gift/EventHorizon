#![cfg(test)]
use super::*;
use soroban_sdk::token::{Client as TokenClient, StellarAssetClient};
use soroban_sdk::{
    testutils::Address as _,
    Address, Env,
};

#[test]
fn test_amm_flow() {
    let env = Env::default();
    env.mock_all_auths();

    // 1. Setup participants
    let user = Address::generate(&env);
    let admin = Address::generate(&env);

    // 2. Register mock tokens
    let token_a_addr = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b_addr = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = StellarAssetClient::new(&env, &token_a_addr);
    let token_b_admin = StellarAssetClient::new(&env, &token_b_addr);

    let token_a = TokenClient::new(&env, &token_a_addr);
    let token_b = TokenClient::new(&env, &token_b_addr);

    // 3. Register AMM contract
    let contract_id = env.register(ConstantProductAMM, ());
    let client = ConstantProductAMMClient::new(&env, &contract_id);

    // 4. Initialize
    client.initialize(&token_a_addr, &token_b_addr);

    // 5. Initial funding
    token_a_admin.mint(&user, &2000);
    token_b_admin.mint(&user, &2000);

    // 6. ADD LIQUIDITY (x=1000, y=1000)
    // lp = sqrt(1000 * 1000) = 1000
    let lp_minted = client.add_liquidity(&user, &1000, &1000);
    assert_eq!(lp_minted, 1000);
    assert_eq!(token_a.balance(&contract_id), 1000);
    assert_eq!(token_b.balance(&contract_id), 1000);

    // 7. SWAP (In=100)
    // dy = 1000 * (100 * 0.997) / (1000 + 100 * 0.997)
    // dy = 1000 * 99.7 / 1099.7 = 99700 / 1099.7 = 90.66 -> 90
    let amt_out = client.swap(&user, &token_a_addr, &100, &90);
    assert_eq!(amt_out, 90);
    assert_eq!(token_a.balance(&contract_id), 1100);
    assert_eq!(token_b.balance(&contract_id), 910);

    // 8. ADD LIQUIDITY SCALE (Subsequent)
    // ratio: 1100 / 910
    // Try add 100 of Token A -> Should optimal B = 100 * 910 / 1100 = 82
    let lp_minted2 = client.add_liquidity(&user, &100, &200);
    assert_eq!(lp_minted2, 90); // (100 * 1000) / 1100 = 90.9 -> 90
    assert_eq!(token_a.balance(&contract_id), 1200);
    assert_eq!(token_b.balance(&contract_id), 910 + 82);

    // 9. REMOVE LIQUIDITY
    let (ra, rb) = client.remove_liquidity(&user, &500);
    assert!(ra > 0);
    assert!(rb > 0);
}

#[test]
#[should_panic(expected = "Slippage limit exceeded")]
fn test_swap_slippage() {
    let env = Env::default();
    env.mock_all_auths();
    let user = Address::generate(&env);
    let admin = Address::generate(&env);
    let token_a_addr = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b_addr = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let contract_id = env.register(ConstantProductAMM, ());
    let client = ConstantProductAMMClient::new(&env, &contract_id);

    client.initialize(&token_a_addr, &token_b_addr);
    StellarAssetClient::new(&env, &token_a_addr).mint(&user, &1000);
    StellarAssetClient::new(&env, &token_b_addr).mint(&user, &1000);

    client.add_liquidity(&user, &1000, &1000);

    // Expected out is 90, we require 95 -> Panic
    client.swap(&user, &token_a_addr, &100, &95);
}
