#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, BytesN, Vec, Symbol, Bytes, xdr::ToXdr};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Token,
    MerkleRoot,
    Expiration,
    Claimed(Address),
}

#[contract]
pub struct AirdropContract;

#[contractimpl]
impl AirdropContract {
    /// Initializes the airdrop contract.
    pub fn initialize(
        env: Env,
        admin: Address,
        token: Address,
        merkle_root: BytesN<32>,
        expiration: u64,
    ) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Token, &token);
        env.storage().instance().set(&DataKey::MerkleRoot, &merkle_root);
        env.storage().instance().set(&DataKey::Expiration, &expiration);
    }

    /// Claims tokens for a user using a Merkle proof.
    pub fn claim(
        env: Env,
        user: Address,
        amount: i128,
        proof: Vec<BytesN<32>>,
    ) {
        user.require_auth();

        // 1. Check expiration
        let expiration: u64 = env.storage().instance().get(&DataKey::Expiration).expect("Not init");
        if env.ledger().timestamp() > expiration {
            panic!("Airdrop expired");
        }

        // 2. Check if already claimed
        if env.storage().persistent().has(&DataKey::Claimed(user.clone())) {
            panic!("Already claimed");
        }

        // 3. Verify Merkle Proof
        let root: BytesN<32> = env.storage().instance().get(&DataKey::MerkleRoot).expect("Not init");
        
        // Leaf = sha256(user_address + amount)
        let mut leaf_data = Bytes::new(&env);
        leaf_data.append(&user.clone().to_xdr(&env));
        leaf_data.append(&amount.clone().to_xdr(&env));
        let leaf = env.crypto().sha256(&leaf_data);

        if !verify_proof(&env, &proof, &root, leaf.into()) {
            panic!("Invalid proof");
        }

        // 4. Mark as claimed
        env.storage().persistent().set(&DataKey::Claimed(user.clone()), &true);

        // 5. Transfer tokens
        let token_addr: Address = env.storage().instance().get(&DataKey::Token).expect("Not init");
        let client = token::Client::new(&env, &token_addr);
        client.transfer(&env.current_contract_address(), &user, &amount);

        // Emit event
        env.events().publish((Symbol::new(&env, "airdrop_claim"), user), amount);
    }

    /// Withdraws remaining tokens after expiration (Admin only).
    pub fn withdraw(env: Env) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).expect("Not init");
        admin.require_auth();

        let expiration: u64 = env.storage().instance().get(&DataKey::Expiration).expect("Not init");
        if env.ledger().timestamp() <= expiration {
            panic!("Airdrop not yet expired");
        }

        let token_addr: Address = env.storage().instance().get(&DataKey::Token).expect("Not init");
        let client = token::Client::new(&env, &token_addr);
        
        let balance = client.balance(&env.current_contract_address());
        if balance > 0 {
            client.transfer(&env.current_contract_address(), &admin, &balance);
        }

        env.events().publish((Symbol::new(&env, "airdrop_withdraw"), admin), balance);
    }

    /// Returns the current airdrop configuration.
    pub fn get_config(env: Env) -> (Address, Address, BytesN<32>, u64) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).expect("Not init");
        let token: Address = env.storage().instance().get(&DataKey::Token).expect("Not init");
        let root: BytesN<32> = env.storage().instance().get(&DataKey::MerkleRoot).expect("Not init");
        let exp: u64 = env.storage().instance().get(&DataKey::Expiration).expect("Not init");
        (admin, token, root, exp)
    }

    /// Checks if a user has already claimed.
    pub fn is_claimed(env: Env, user: Address) -> bool {
        env.storage().persistent().has(&DataKey::Claimed(user))
    }
}

/// Helper function to verify a Merkle proof.
fn verify_proof(env: &Env, proof: &Vec<BytesN<32>>, root: &BytesN<32>, leaf: BytesN<32>) -> bool {
    let mut computed_hash = leaf;

    for p in proof.iter() {
        let mut data = Bytes::new(env);
        if computed_hash < p {
            data.append(&computed_hash.clone().into());
            data.append(&p.clone().into());
        } else {
            data.append(&p.clone().into());
            data.append(&computed_hash.clone().into());
        }
        computed_hash = env.crypto().sha256(&data).into();
    }

    computed_hash == *root
}
