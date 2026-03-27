#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger};
    use soroban_sdk::{token, Address, Env};

    #[test]
    fn test_subscription_flow() {
        let env = Env::default();
        env.mock_all_auths();

        let subscriber = Address::generate(&env);
        let provider = Address::generate(&env);
        
        // Register mock token
        let token_addr = env.register_stellar_asset_contract_v2(subscriber.clone()).address();
        let token_admin = token::StellarAssetClient::new(&env, &token_addr);
        let token_client = token::Client::new(&env, &token_addr);

        // Register subscription contract
        let contract_id = env.register_contract(None, SubscriptionContract);
        let client = SubscriptionContractClient::new(&env, &contract_id);

        // Setup: Funding and Allowance
        let amount = 100;
        let frequency = 3600; // 1 hour
        token_admin.mint(&subscriber, &1000);
        
        // Subscriber must approve the contract to spend tokens
        token_client.approve(&subscriber, &contract_id, &1000, &1000);

        // 1. Create Subscription
        let sub_id = client.create_subscription(&subscriber, &provider, &token_addr, &amount, &frequency);

        // 2. Process First Payment
        client.process_payment(&sub_id);
        assert_eq!(token_client.balance(&subscriber), 900);
        assert_eq!(token_client.balance(&provider), 100);

        // Verify state
        let sub = client.get_subscription(&sub_id);
        assert_eq!(sub.last_payment, 0); // Oops, I set sub.last_payment = now in lib.rs
        // Actually, env.ledger().timestamp() defaults to 0 in tests unless set.
    }

    #[test]
    #[should_panic(expected = "Wait until the next billing cycle")]
    fn test_early_payment_fails() {
        let env = Env::default();
        env.mock_all_auths();

        let subscriber = Address::generate(&env);
        let provider = Address::generate(&env);
        let token_addr = env.register_stellar_asset_contract_v2(subscriber.clone()).address();
        let token_admin = token::StellarAssetClient::new(&env, &token_addr);
        let token_client = token::Client::new(&env, &token_addr);

        let contract_id = env.register_contract(None, SubscriptionContract);
        let client = SubscriptionContractClient::new(&env, &contract_id);

        token_admin.mint(&subscriber, &1000);
        token_client.approve(&subscriber, &contract_id, &1000, &1000);

        let sub_id = client.create_subscription(&subscriber, &provider, &token_addr, &100, &3600);
        
        client.process_payment(&sub_id); // First one ok (timestamp 0)
        
        // Try to process again at same timestamp (0)
        client.process_payment(&sub_id); // Second one fails because now (0) < last (0) + 3600??
        // Wait, 0 < 0 + 3600 is true. So it should panic.
    }

    #[test]
    fn test_recurring_payment_success() {
        let env = Env::default();
        env.mock_all_auths();

        let subscriber = Address::generate(&env);
        let provider = Address::generate(&env);
        let token_addr = env.register_stellar_asset_contract_v2(subscriber.clone()).address();
        let token_admin = token::StellarAssetClient::new(&env, &token_addr);
        let token_client = token::Client::new(&env, &token_addr);

        let contract_id = env.register_contract(None, SubscriptionContract);
        let client = SubscriptionContractClient::new(&env, &contract_id);

        token_admin.mint(&subscriber, &1000);
        token_client.approve(&subscriber, &contract_id, &1000, &1000);

        let sub_id = client.create_subscription(&subscriber, &provider, &token_addr, &100, &3600);
        
        client.process_payment(&sub_id); // timestamp 0
        
        // Fast forward 1 hour
        env.ledger().set_timestamp(3601);
        client.process_payment(&sub_id);

        assert_eq!(token_client.balance(&subscriber), 800);
        assert_eq!(token_client.balance(&provider), 200);
    }

    #[test]
    fn test_lifecycle_management() {
        let env = Env::default();
        env.mock_all_auths();

        let subscriber = Address::generate(&env);
        let provider = Address::generate(&env);
        let token_addr = env.register_stellar_asset_contract_v2(subscriber.clone()).address();
        let contract_id = env.register_contract(None, SubscriptionContract);
        let client = SubscriptionContractClient::new(&env, &contract_id);

        let sub_id = client.create_subscription(&subscriber, &provider, &token_addr, &100, &3600);
        
        // Pause
        client.pause_subscription(&sub_id);
        let mut sub = client.get_subscription(&sub_id);
        assert_eq!(sub.status, SubscriptionStatus::Paused);

        // Resume
        client.resume_subscription(&sub_id);
        sub = client.get_subscription(&sub_id);
        assert_eq!(sub.status, SubscriptionStatus::Active);

        // Cancel
        client.cancel_subscription(&sub_id);
        sub = client.get_subscription(&sub_id);
        assert_eq!(sub.status, SubscriptionStatus::Cancelled);
    }

    #[test]
    #[should_panic(expected = "Subscription is not active")]
    fn test_payment_on_cancelled_fails() {
        let env = Env::default();
        env.mock_all_auths();

        let subscriber = Address::generate(&env);
        let provider = Address::generate(&env);
        let token_addr = env.register_stellar_asset_contract_v2(subscriber.clone()).address();
        let contract_id = env.register_contract(None, SubscriptionContract);
        let client = SubscriptionContractClient::new(&env, &contract_id);

        let sub_id = client.create_subscription(&subscriber, &provider, &token_addr, &100, &3600);
        client.cancel_subscription(&sub_id);
        
        client.process_payment(&sub_id);
    }
}
