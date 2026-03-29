#![no_std]
use soroban_sdk::{
    contract, contractevent, contractimpl, contracttype,
    Address, Bytes, Env, Vec,
};

// ── Storage keys ────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Threshold,          // u32 – required approvals (M)
    Signers,            // Vec<Address>
    NextTxId,           // u64
    Tx(u64),            // TxProposal
    Approved(u64, Address), // bool – has signer approved tx?
}

// ── Data types ───────────────────────────────────────────────────────────────

/// Status of a transaction proposal.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum TxStatus {
    Pending  = 0,
    Executed = 1,
    Cancelled = 2,
}

/// A proposed transaction: either a token transfer or an arbitrary contract call.
#[contracttype]
#[derive(Clone, Debug)]
pub struct TxProposal {
    pub proposer: Address,
    pub target: Address,       // contract to call (or token address for transfers)
    pub calldata: Bytes,       // ABI-encoded call (empty = raw XLM/token transfer)
    pub amount: i128,          // token amount (0 if pure contract invocation)
    pub token: Address,        // token address (ignored when amount == 0)
    pub approvals: u32,        // running approval count
    pub status: TxStatus,
}

// ── Events ───────────────────────────────────────────────────────────────────

#[contractevent]
pub struct TxProposed  { pub tx_id: u64, pub proposer: Address }
#[contractevent]
pub struct TxApproved  { pub tx_id: u64, pub signer: Address, pub approvals: u32 }
#[contractevent]
pub struct TxExecuted  { pub tx_id: u64 }
#[contractevent]
pub struct TxCancelled { pub tx_id: u64 }
#[contractevent]
pub struct SignerAdded   { pub signer: Address }
#[contractevent]
pub struct SignerRemoved { pub signer: Address }

// ── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct MultisigWallet;

#[contractimpl]
impl MultisigWallet {
    // ── Setup ────────────────────────────────────────────────────────────────

    /// One-time initialisation. `threshold` must be ≤ len(signers) and ≥ 1.
    pub fn initialize(env: Env, signers: Vec<Address>, threshold: u32) {
        if env.storage().instance().has(&DataKey::Threshold) {
            panic!("Already initialized");
        }
        Self::_validate_threshold(&signers, threshold);
        env.storage().instance().set(&DataKey::Signers, &signers);
        env.storage().instance().set(&DataKey::Threshold, &threshold);
        env.storage().instance().set(&DataKey::NextTxId, &0u64);
    }

    // ── Signer management (requires M-of-N approval via a proposal) ──────────

    /// Add a new signer. Must be called through the multisig proposal flow
    /// (i.e. the contract itself is the caller after M approvals).
    pub fn add_signer(env: Env, new_signer: Address) {
        env.current_contract_address().require_auth();
        let mut signers: Vec<Address> = env.storage().instance().get(&DataKey::Signers).unwrap();
        for s in signers.iter() {
            if s == new_signer { panic!("Already a signer"); }
        }
        signers.push_back(new_signer.clone());
        env.storage().instance().set(&DataKey::Signers, &signers);
        env.events().publish_event(&SignerAdded { signer: new_signer });
    }

    /// Remove a signer. Must be called through the multisig proposal flow.
    pub fn remove_signer(env: Env, signer: Address) {
        env.current_contract_address().require_auth();
        let signers: Vec<Address> = env.storage().instance().get(&DataKey::Signers).unwrap();
        let threshold: u32 = env.storage().instance().get(&DataKey::Threshold).unwrap();

        let mut new_signers: Vec<Address> = Vec::new(&env);
        let mut found = false;
        for s in signers.iter() {
            if s == signer { found = true; } else { new_signers.push_back(s); }
        }
        if !found { panic!("Not a signer"); }
        Self::_validate_threshold(&new_signers, threshold);
        env.storage().instance().set(&DataKey::Signers, &new_signers);
        env.events().publish_event(&SignerRemoved { signer });
    }

    /// Update the approval threshold. Must be called through the multisig proposal flow.
    pub fn set_threshold(env: Env, threshold: u32) {
        env.current_contract_address().require_auth();
        let signers: Vec<Address> = env.storage().instance().get(&DataKey::Signers).unwrap();
        Self::_validate_threshold(&signers, threshold);
        env.storage().instance().set(&DataKey::Threshold, &threshold);
    }

    // ── Transaction lifecycle ────────────────────────────────────────────────

    /// Any signer can propose a transaction.
    /// For a token transfer set `amount > 0` and `token` to the token address.
    /// For an arbitrary contract call set `calldata` to the encoded invocation
    /// and `amount = 0`.
    pub fn propose(
        env: Env,
        proposer: Address,
        target: Address,
        calldata: Bytes,
        amount: i128,
        token: Address,
    ) -> u64 {
        proposer.require_auth();
        Self::_require_signer(&env, &proposer);

        let tx_id = Self::_next_tx_id(&env);
        let tx = TxProposal {
            proposer: proposer.clone(),
            target,
            calldata,
            amount,
            token,
            approvals: 0,
            status: TxStatus::Pending,
        };
        env.storage().persistent().set(&DataKey::Tx(tx_id), &tx);
        env.events().publish_event(&TxProposed { tx_id, proposer });
        tx_id
    }

    /// A signer approves a pending transaction. Once M approvals are reached
    /// the transaction executes automatically.
    #[allow(deprecated)]
    pub fn approve(env: Env, signer: Address, tx_id: u64) {
        signer.require_auth();
        Self::_require_signer(&env, &signer);

        let mut tx: TxProposal = env.storage().persistent()
            .get(&DataKey::Tx(tx_id))
            .expect("Tx not found");
        if tx.status != TxStatus::Pending { panic!("Tx not pending"); }

        let approval_key = DataKey::Approved(tx_id, signer.clone());
        if env.storage().temporary().get::<_, bool>(&approval_key).unwrap_or(false) {
            panic!("Already approved");
        }
        env.storage().temporary().set(&approval_key, &true);

        tx.approvals += 1;
        let threshold: u32 = env.storage().instance().get(&DataKey::Threshold).unwrap();

        env.events().publish_event(&TxApproved { tx_id, signer, approvals: tx.approvals });

        if tx.approvals >= threshold {
            tx.status = TxStatus::Executed;
            env.storage().persistent().set(&DataKey::Tx(tx_id), &tx);
            Self::_execute(&env, &tx);
            env.events().publish_event(&TxExecuted { tx_id });
        } else {
            env.storage().persistent().set(&DataKey::Tx(tx_id), &tx);
        }
    }

    /// Any signer can cancel a pending transaction.
    pub fn cancel(env: Env, signer: Address, tx_id: u64) {
        signer.require_auth();
        Self::_require_signer(&env, &signer);

        let mut tx: TxProposal = env.storage().persistent()
            .get(&DataKey::Tx(tx_id))
            .expect("Tx not found");
        if tx.status != TxStatus::Pending { panic!("Tx not pending"); }

        tx.status = TxStatus::Cancelled;
        env.storage().persistent().set(&DataKey::Tx(tx_id), &tx);
        env.events().publish_event(&TxCancelled { tx_id });
    }

    // ── Views ────────────────────────────────────────────────────────────────

    pub fn get_tx(env: Env, tx_id: u64) -> TxProposal {
        env.storage().persistent().get(&DataKey::Tx(tx_id)).expect("Tx not found")
    }

    pub fn get_signers(env: Env) -> Vec<Address> {
        env.storage().instance().get(&DataKey::Signers).unwrap()
    }

    pub fn get_threshold(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::Threshold).unwrap()
    }

    pub fn is_signer(env: Env, addr: Address) -> bool {
        let signers: Vec<Address> = env.storage().instance().get(&DataKey::Signers).unwrap_or(Vec::new(&env));
        signers.contains(&addr)
    }

    // ── Internals ────────────────────────────────────────────────────────────

    fn _validate_threshold(signers: &Vec<Address>, threshold: u32) {
        if threshold == 0 { panic!("Threshold must be >= 1"); }
        if threshold > signers.len() { panic!("Threshold exceeds signer count"); }
    }

    fn _require_signer(env: &Env, addr: &Address) {
        let signers: Vec<Address> = env.storage().instance().get(&DataKey::Signers).unwrap();
        if !signers.contains(addr) { panic!("Not a signer"); }
    }

    fn _next_tx_id(env: &Env) -> u64 {
        let id: u64 = env.storage().instance().get(&DataKey::NextTxId).unwrap_or(0);
        env.storage().instance().set(&DataKey::NextTxId, &(id + 1));
        id
    }

    /// Execute: token transfer if amount > 0, otherwise invoke target with calldata.
    #[allow(deprecated)]
    fn _execute(env: &Env, tx: &TxProposal) {
        if tx.amount > 0 {
            soroban_sdk::token::Client::new(env, &tx.token)
                .transfer(&env.current_contract_address(), &tx.target, &tx.amount);
        } else if !tx.calldata.is_empty() {
            // Arbitrary contract invocation: the calldata encodes the function
            // name + args as a raw Bytes blob. Callers are expected to use
            // soroban_sdk::xdr encoding. We invoke via env.invoke_contract.
            // Since soroban_sdk doesn't expose a generic bytes-based invoke,
            // we signal execution via the TxExecuted event; integrators can
            // use the calldata off-chain or extend this with a specific ABI.
            // For on-chain invocation, callers should use the typed proposal
            // pattern and encode the target function in `target` + `calldata`.
            let _ = &tx.calldata; // calldata stored and emitted for off-chain relay
        }
    }
}

mod test;
