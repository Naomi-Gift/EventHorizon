#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String, Symbol, Vec,
};

// ── Events ──────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoyaltyPaid {
    pub token_id: u32,
    pub recipient: Address,
    pub amount: i128,
    pub asset: Address,
    pub sender: Address,
}

// ── Storage & Types ─────────────────────────────────────────────────────────

/// Royalty info stored per token (index) — shares expressed as basis points (0–10000).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoyaltyInfo {
    /// Recipient of royalty payments.
    pub recipient: Address,
    /// Basis points (e.g. 500 = 5%).
    pub bps: u32,
}

/// Per-token metadata entry.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenMetadata {
    /// Off-chain metadata URI (e.g. IPFS link).
    pub uri: String,
}

#[contracttype]
enum DataKey {
    Admin,
    Minter,
    Name,
    Symbol,
    NextTokenId,
    TokenOwner(u32),
    TokenMetadata(u32),
    TokenRoyalty(u32),
    DefaultRoyalty,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    InvalidRoyaltyBps = 4,
    TokenNotFound = 5,
    NotTokenOwner = 6,
    Overflow = 7,
}

// ── Contract ────────────────────────────────────────────────────────────────

#[contract]
pub struct NftFactory;

#[contractimpl]
impl NftFactory {
    /// Initialise the NFT collection.
    pub fn initialize(
        env: Env,
        admin: Address,
        minter: Address,
        name: String,
        symbol: Symbol,
        default_royalty_bps: u32,
    ) {
        if env.storage().instance().has(&DataKey::Admin) {
            env.panic_with_error(Error::AlreadyInitialized);
        }
        if default_royalty_bps > 10_000 {
            env.panic_with_error(Error::InvalidRoyaltyBps);
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Minter, &minter);
        env.storage().instance().set(&DataKey::Name, &name);
        env.storage().instance().set(&DataKey::Symbol, &symbol);
        env.storage().instance().set(&DataKey::NextTokenId, &0u32);
        env.storage().instance().set(
            &DataKey::DefaultRoyalty,
            &RoyaltyInfo {
                recipient: admin.clone(),
                bps: default_royalty_bps,
            },
        );
    }

    // ── Minting ─────────────────────────────────────────────────────────

    /// Mint a single NFT. Only the Minter can call this.
    pub fn mint(env: Env, to: Address, uri: String, royalty_bps: Option<u32>) -> u32 {
        let minter: Address = env.storage().instance().get(&DataKey::Minter).expect("not initialized");
        minter.require_auth();

        let mut next_id: u32 = env.storage().instance().get(&DataKey::NextTokenId).unwrap_or(0);
        let token_id = next_id;
        next_id = next_id.checked_add(1).unwrap_or_else(|| env.panic_with_error(Error::Overflow));

        env.storage().instance().set(&DataKey::NextTokenId, &next_id);
        env.storage().instance().set(&DataKey::TokenOwner(token_id), &to);
        env.storage().instance().set(&DataKey::TokenMetadata(token_id), &TokenMetadata { uri: uri.clone() });

        if let Some(bps) = royalty_bps {
            if bps > 10_000 {
                env.panic_with_error(Error::InvalidRoyaltyBps);
            }
            env.storage().instance().set(
                &DataKey::TokenRoyalty(token_id),
                &RoyaltyInfo {
                    recipient: to.clone(),
                    bps,
                },
            );
        }

        // Emit Standardized Events
        // Topic: (Transfer, from, to)
        env.events().publish(
            (symbol_short!("Transfer"), Address::generate(&env), to.clone()), 
            token_id,
        );
        // Topic: (MetadataUpdated, token_id)
        env.events().publish(
            (Symbol::new(&env, "MetadataUpdated"), token_id),
            uri,
        );

        token_id
    }

    /// Batch-mint several NFTs at once.
    pub fn batch_mint(env: Env, to: Address, uris: Vec<String>, royalty_bps: Option<u32>) -> Vec<u32> {
        let minter: Address = env.storage().instance().get(&DataKey::Minter).expect("not initialized");
        minter.require_auth();

        let mut next_id: u32 = env.storage().instance().get(&DataKey::NextTokenId).unwrap_or(0);
        let mut ids = Vec::new(&env);

        for uri_res in uris.iter() {
            let uri = uri_res;
            let token_id = next_id;
            next_id = next_id.checked_add(1).unwrap_or_else(|| env.panic_with_error(Error::Overflow));

            env.storage().instance().set(&DataKey::TokenOwner(token_id), &to.clone());
            env.storage().instance().set(&DataKey::TokenMetadata(token_id), &TokenMetadata { uri: uri.clone() });

            if let Some(bps) = royalty_bps {
                env.storage().instance().set(
                    &DataKey::TokenRoyalty(token_id),
                    &RoyaltyInfo {
                        recipient: to.clone(),
                        bps,
                    },
                );
            }

            env.events().publish(
                (symbol_short!("Transfer"), Address::generate(&env), to.clone()),
                token_id,
            );
            env.events().publish(
                (Symbol::new(&env, "MetadataUpdated"), token_id),
                uri,
            );

            ids.push_back(token_id);
        }

        env.storage().instance().set(&DataKey::NextTokenId, &next_id);
        ids
    }

    // ── Transfers ───────────────────────────────────────────────────────

    /// Transfer an NFT to a new owner.
    pub fn transfer(env: Env, from: Address, to: Address, token_id: u32) {
        from.require_auth();

        let owner: Address = env
            .storage()
            .instance()
            .get(&DataKey::TokenOwner(token_id))
            .unwrap_or_else(|| env.panic_with_error(Error::TokenNotFound));

        if owner != from {
            env.panic_with_error(Error::NotTokenOwner);
        }

        env.storage().instance().set(&DataKey::TokenOwner(token_id), &to);

        env.events().publish(
            (symbol_short!("Transfer"), from, to),
            token_id,
        );
    }

    /// Transfer multiple NFTs at once.
    /// transfers: Vec<(to, token_id)>
    pub fn batch_transfer(env: Env, from: Address, transfers: Vec<(Address, u32)>) {
        from.require_auth();

        for transfer_item_res in transfers.iter() {
            let (to, token_id) = transfer_item_res;
            
            let owner: Address = env
                .storage()
                .instance()
                .get(&DataKey::TokenOwner(token_id))
                .unwrap_or_else(|| env.panic_with_error(Error::TokenNotFound));

            if owner != from {
                env.panic_with_error(Error::NotTokenOwner);
            }

            env.storage().instance().set(&DataKey::TokenOwner(token_id), &to);

            env.events().publish(
                (symbol_short!("Transfer"), from.clone(), to),
                token_id,
            );
        }
    }

    // ── Metadata & Royalty ──────────────────────────────────────────────

    /// Update metadata URI (only token owner).
    pub fn update_metadata(env: Env, owner: Address, token_id: u32, new_uri: String) {
        owner.require_auth();

        let current_owner: Address = env
            .storage()
            .instance()
            .get(&DataKey::TokenOwner(token_id))
            .unwrap_or_else(|| env.panic_with_error(Error::TokenNotFound));

        if current_owner != owner {
            env.panic_with_error(Error::NotTokenOwner);
        }

        env.storage().instance().set(&DataKey::TokenMetadata(token_id), &TokenMetadata { uri: new_uri.clone() });

        env.events().publish(
            (Symbol::new(&env, "MetadataUpdated"), token_id),
            new_uri,
        );
    }

    /// Record a royalty payment event.
    pub fn pay_royalty(env: Env, sender: Address, token_id: u32, amount: i128, asset: Address) {
        sender.require_auth();

        let info = env.storage().instance().get(&DataKey::TokenRoyalty(token_id))
            .unwrap_or_else(|| {
                env.storage().instance().get(&DataKey::DefaultRoyalty)
                    .unwrap_or_else(|| env.panic_with_error(Error::NotInitialized))
            });
        
        env.events().publish(
            (Symbol::new(&env, "RoyaltyPaid"), token_id, info.recipient.clone()),
            RoyaltyPaid {
                token_id,
                recipient: info.recipient,
                amount,
                asset,
                sender,
            },
        );
    }

    // ── Admin Functions ────────────────────────────────────────────────

    pub fn set_admin(env: Env, admin: Address, new_admin: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            env.panic_with_error(Error::Unauthorized);
        }
        env.storage().instance().set(&DataKey::Admin, &new_admin);
    }

    pub fn set_minter(env: Env, admin: Address, new_minter: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            env.panic_with_error(Error::Unauthorized);
        }
        env.storage().instance().set(&DataKey::Minter, &new_minter);
    }

    pub fn set_default_royalty(env: Env, admin: Address, recipient: Address, bps: u32) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            env.panic_with_error(Error::Unauthorized);
        }
        if bps > 10_000 {
            env.panic_with_error(Error::InvalidRoyaltyBps);
        }
        env.storage()
            .instance()
            .set(&DataKey::DefaultRoyalty, &RoyaltyInfo { recipient, bps });
    }

    // ── Queries ────────────────────────────────────────────────────────

    pub fn owner_of(env: Env, token_id: u32) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::TokenOwner(token_id))
            .unwrap_or_else(|| env.panic_with_error(Error::TokenNotFound))
    }

    pub fn royalty_info(env: Env, token_id: u32) -> RoyaltyInfo {
        env.storage()
            .instance()
            .get(&DataKey::TokenRoyalty(token_id))
            .unwrap_or_else(|| {
                env.storage()
                    .instance()
                    .get(&DataKey::DefaultRoyalty)
                    .unwrap_or_else(|| env.panic_with_error(Error::NotInitialized))
            })
    }

    pub fn token_metadata(env: Env, token_id: u32) -> TokenMetadata {
        env.storage()
            .instance()
            .get(&DataKey::TokenMetadata(token_id))
            .unwrap_or_else(|| env.panic_with_error(Error::TokenNotFound))
    }

    pub fn name(env: Env) -> String {
        env.storage().instance().get(&DataKey::Name).unwrap()
    }

    pub fn symbol(env: Env) -> Symbol {
        env.storage().instance().get(&DataKey::Symbol).unwrap()
    }

    pub fn total_supply(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::NextTokenId).unwrap_or(0)
    }
}

mod test;
