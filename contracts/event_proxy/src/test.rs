#![cfg(test)]
use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Bytes, Env, Vec,
};

// Helper to create N signers
fn make_signers(env: &Env, n: u32) -> Vec<Address> {
    let mut v = Vec::new(env);
    for _ in 0..n { v.push_back(Address::generate(env)); }
    v
}

// Setup 2-of-3 multisig with 1 hour timelock
fn setup_2of3(env: &Env) -> (Address, Vec<Address>) {
    let signers = make_signers(env, 3);
    let contract_id = env.register(EventProxy, ());
    let client = EventProxyClient::new(env, &contract_id);
    client.initialize(&signers, &2, &3600); // 1 hour = 3600 seconds
    (contract_id, signers)
}

#[test]
fn test_initialization_and_threshold() {
    let env = Env::default();
    env.mock_all_auths();

    let signers = make_signers(&env, 3);
    let contract_id = env.register(EventProxy, ());
    let client = EventProxyClient::new(&env, &contract_id);

    // Initialize
    client.initialize(&signers, &2, &3600);
    assert_eq!(client.get_threshold(), 2);
    assert_eq!(client.get_signers().len(), 3);
    assert_eq!(client.get_timelock_delay(), 3600);
}

#[test]
#[should_panic(expected = "Event not found")]
fn test_get_nonexistent_event_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(EventProxy, ());
    let _client = EventProxyClient::new(&env, &contract_id);
    let _ = EventProxyClient::new(&env, &contract_id).get_event(&999);
}

#[test]
#[should_panic(expected = "Threshold exceeds signer count")]
fn test_threshold_exceeds_signers() {
    let env = Env::default();
    env.mock_all_auths();
    let signers = make_signers(&env, 2);
    let contract_id = env.register(EventProxy, ());
    let client = EventProxyClient::new(&env, &contract_id);
    client.initialize(&signers, &3, &3600);
}

#[test]
#[should_panic(expected = "Timelock delay must be > 0")]
fn test_zero_timelock_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let signers = make_signers(&env, 2);
    let contract_id = env.register(EventProxy, ());
    let client = EventProxyClient::new(&env, &contract_id);
    client.initialize(&signers, &1, &0);
}

#[test]
#[should_panic(expected = "Must have at least 1 signer")]
fn test_empty_signers_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let signers = Vec::new(&env); // empty
    let contract_id = env.register(EventProxy, ());
    let client = EventProxyClient::new(&env, &contract_id);
    client.initialize(&signers, &1, &3600);
}

// ── Event lifecycle tests ────────────────────────────────────────────────────

#[test]
fn test_event_lifecycle_full_flow() {
    let env = Env::default();
    env.mock_all_auths();

    let (contract_id, signers) = setup_2of3(&env);
    let client = EventProxyClient::new(&env, &contract_id);

    // Prepare a token transfer event
    let token_addr = env.register_stellar_asset_contract_v2(signers.get(0).unwrap().clone()).address();
    let recipient = Address::generate(&env);
    let empty = Bytes::new(&env);

    // Step 1: Schedule event (proposer is signer[0])
    let event_id = client.schedule_event(
        &signers.get(0).unwrap(),
        &recipient,
        &empty,
        &500i128,
        &token_addr,
    );
    assert_eq!(event_id, 0);
    let event = client.get_event(&event_id);
    assert_eq!(event.status, EventStatus::Pending);
    assert_eq!(event.approvals, 0);
    assert_eq!(event.amount, 500);
    assert_eq!(event.target, recipient);

    // Step 2: First approval (approvals = 1, threshold = 2, still pending)
    client.approve(&signers.get(0).unwrap(), &event_id);
    let event = client.get_event(&event_id);
    assert_eq!(event.approvals, 1);
    assert_eq!(event.status, EventStatus::Pending);

    // Step 3: Second approval → threshold reached → status becomes Queued with timelock
    client.approve(&signers.get(1).unwrap(), &event_id);
    let event = client.get_event(&event_id);
    assert_eq!(event.approvals, 2);
    assert_eq!(event.status, EventStatus::Queued);
    assert!(event.execution_time > event.queued_at);
    assert_eq!(event.execution_time, event.queued_at + 3600);

    // Step 4: Move time past timelock and execute
    env.ledger().set_timestamp(event.execution_time + 1);
    client.execute(&event_id);
    let event = client.get_event(&event_id);
    assert_eq!(event.status, EventStatus::Executed);
    // Check recipient received tokens
    assert_eq!(TokenClient::new(&env, &token_addr).balance(&recipient), 500);
}

#[test]
#[should_panic(expected = "Timelock not expired")]
fn test_execute_before_timelock_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let (contract_id, signers) = setup_2of3(&env);
    let client = EventProxyClient::new(&env, &contract_id);

    let token_addr = env.register_stellar_asset_contract_v2(signers.get(0).unwrap().clone()).address();
    let recipient = Address::generate(&env);
    let empty = Bytes::new(&env);

    let event_id = client.schedule_event(&signers.get(0).unwrap(), &recipient, &empty, &100i128, &token_addr);
    client.approve(&signers.get(0).unwrap(), &event_id);
    client.approve(&signers.get(1).unwrap(), &event_id);

    let event = client.get_event(&event_id);
    assert_eq!(event.status, EventStatus::Queued);

    // Try to execute immediately → should panic (timelock not expired)
    client.execute(&event_id);
}

#[test]
fn test_timelock_requires_wait() {
    let env = Env::default();
    env.mock_all_auths();

    let (contract_id, signers) = setup_2of3(&env);
    let client = EventProxyClient::new(&env, &contract_id);

    let token_addr = env.register_stellar_asset_contract_v2(signers.get(0).unwrap().clone()).address();
    let recipient = Address::generate(&env);
    let empty = Bytes::new(&env);

    let event_id = client.schedule_event(&signers.get(0).unwrap(), &recipient, &empty, &100i128, &token_addr);
    client.approve(&signers.get(0).unwrap(), &event_id);
    client.approve(&signers.get(1).unwrap(), &event_id);

    let event = client.get_event(&event_id);
    assert_eq!(event.status, EventStatus::Queued);
    assert!(event.execution_time > event.queued_at);

    // After timelock: execution_time + 1
    env.ledger().set_timestamp(event.execution_time + 1);
    client.execute(&event_id);
    assert_eq!(client.get_event(&event_id).status, EventStatus::Executed);
}
