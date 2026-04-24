#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::{Address as _, Events}, Address, String, Vec, IntoVal, Symbol};

#[test]
fn test_lifecycle_and_events() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let minter = Address::generate(&env);
    let user = Address::generate(&env);
    let recipient = Address::generate(&env);

    let contract_id = env.register(NftFactory, ());
    let client = NftFactoryClient::new(&env, &contract_id);

    // 1. Initialize
    client.initialize(
        &admin,
        &minter,
        &String::from_str(&env, "EventHorizon NFTs"),
        &symbol_short!("EH"),
        &500,
    );

    // 2. Mint
    let uri = String::from_str(&env, "ipfs://QmExample");
    let token_id = client.mint(&user, &uri, &Some(750));

    assert_eq!(token_id, 0);
    assert_eq!(client.owner_of(&token_id), user);

    // Check Events (Transfer and MetadataUpdated)
    let events = env.events().all();
    let transfer_event = events.get(0).unwrap();
    assert_eq!(transfer_event.0, contract_id.clone());
    assert_eq!(transfer_event.1.get(0).unwrap(), symbol_short!("Transfer").into_val(&env));
    
    let metadata_event = events.get(1).unwrap();
    assert_eq!(metadata_event.1.get(0).unwrap(), Symbol::new(&env, "MetadataUpdated").into_val(&env));
    assert_eq!(metadata_event.1.get(1).unwrap(), 0u32.into_val(&env));
    assert_eq!(metadata_event.2, uri.into_val(&env));

    // 3. Batch Mint
    let mut uris = Vec::new(&env);
    uris.push_back(String::from_str(&env, "ipfs://a"));
    uris.push_back(String::from_str(&env, "ipfs://b"));
    client.batch_mint(&user, &uris, &None);

    assert_eq!(client.total_supply(), 3);

    // 4. Transfer
    client.transfer(&user, &recipient, &0);
    assert_eq!(client.owner_of(&0), recipient);

    // 5. Batch Transfer
    let mut transfers = Vec::new(&env);
    transfers.push_back((recipient.clone(), 1u32));
    transfers.push_back((recipient.clone(), 2u32));
    client.batch_transfer(&user, &transfers);
    assert_eq!(client.owner_of(&1), recipient);
    assert_eq!(client.owner_of(&2), recipient);

    // 6. Royalty Paid Event
    let asset = Address::generate(&env);
    client.pay_royalty(&recipient, &0, &1000i128, &asset);

    let all_events = env.events().all();
    let last_event = all_events.get(all_events.len() - 1).unwrap();
    assert_eq!(last_event.1.get(0).unwrap(), Symbol::new(&env, "RoyaltyPaid").into_val(&env));
}

#[test]
#[should_panic(expected = "Error(NotTokenOwner)")]
fn test_unauthorized_transfer() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let minter = Address::generate(&env);
    let user = Address::generate(&env);
    let hacker = Address::generate(&env);

    let contract_id = env.register(NftFactory, ());
    let client = NftFactoryClient::new(&env, &contract_id);

    client.initialize(&admin, &minter, &String::from_str(&env, "Test"), &symbol_short!("T"), &0);
    client.mint(&user, &String::from_str(&env, "uri"), &None);

    // Hacker tries to transfer user's token
    client.transfer(&hacker, &hacker, &0);
}

#[test]
fn test_admin_functions() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let minter = Address::generate(&env);

    let contract_id = env.register(NftFactory, ());
    let client = NftFactoryClient::new(&env, &contract_id);

    client.initialize(&admin, &minter, &String::from_str(&env, "Test"), &symbol_short!("T"), &0);

    client.set_admin(&admin, &new_admin);
}
