#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, Symbol, Vec};

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum EscrowStatus {
    Active = 0,
    Disputed = 1,
    Released = 2,
    Cancelled = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowInfo {
    pub sender: Address,
    pub recipient: Address,
    pub arbitrator: Address,
    pub token: Address,
    pub amount: i128,
    pub unlock_time: u64,
    pub status: EscrowStatus,
    pub evidence_hash: Vec<u8>,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    PlatformFeeAccount,
    PlatformFeeBps,
    ArbitratorFeeBps,
    NextId,
    Escrow(u64),
}

#[contract]
pub struct EscrowContract;

#[contractimpl]
impl EscrowContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        platform_fee_account: Address,
        platform_fee_bps: u32,
        arbitrator_fee_bps: u32,
    ) {
        admin.require_auth();
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }
        Self::assert_fee_config(platform_fee_bps, arbitrator_fee_bps);
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::PlatformFeeAccount, &platform_fee_account);
        env.storage().instance().set(&DataKey::PlatformFeeBps, &platform_fee_bps);
        env.storage().instance().set(&DataKey::ArbitratorFeeBps, &arbitrator_fee_bps);
    }

    pub fn set_fee_config(
        env: Env,
        admin: Address,
        platform_fee_account: Address,
        platform_fee_bps: u32,
        arbitrator_fee_bps: u32,
    ) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).expect("Not initialized");
        if admin != stored_admin {
            panic!("Not authorized");
        }
        Self::assert_fee_config(platform_fee_bps, arbitrator_fee_bps);
        env.storage().instance().set(&DataKey::PlatformFeeAccount, &platform_fee_account);
        env.storage().instance().set(&DataKey::PlatformFeeBps, &platform_fee_bps);
        env.storage().instance().set(&DataKey::ArbitratorFeeBps, &arbitrator_fee_bps);
    }

    pub fn initiate_escrow(
        env: Env,
        sender: Address,
        recipient: Address,
        arbitrator: Address,
        token: Address,
        amount: i128,
        unlock_time: u64,
    ) -> u64 {
        sender.require_auth();
        if amount <= 0 {
            panic!("Amount must be positive");
        }
        if unlock_time <= env.ledger().timestamp() {
            panic!("Unlock time must be in future");
        }
        if recipient == sender || arbitrator == sender || arbitrator == recipient {
            panic!("Invalid escrow participants");
        }

        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&sender, &env.current_contract_address(), &amount);

        let mut next_id: u64 = env.storage().instance().get(&DataKey::NextId).unwrap_or(1);
        let escrow_id = next_id;
        next_id += 1;
        env.storage().instance().set(&DataKey::NextId, &next_id);

        let escrow = EscrowInfo {
            sender: sender.clone(),
            recipient: recipient.clone(),
            arbitrator: arbitrator.clone(),
            token: token.clone(),
            amount,
            unlock_time,
            status: EscrowStatus::Active,
            evidence_hash: Vec::new(&env),
        };

        env.storage().persistent().set(&DataKey::Escrow(escrow_id), &escrow);

        env.events().publish(
            (Symbol::new(&env, "escrow_created"), escrow_id, sender, recipient, arbitrator),
            (token, amount, unlock_time),
        );

        escrow_id
    }

    pub fn dispute_escrow(env: Env, escrow_id: u64, caller: Address, evidence_hash: Vec<u8>) {
        caller.require_auth();
        let mut escrow = Self::load_escrow(&env, escrow_id);

        if escrow.status != EscrowStatus::Active {
            panic!("Escrow cannot be disputed");
        }
        if caller != escrow.sender && caller != escrow.recipient {
            panic!("Only sender or recipient can dispute");
        }

        escrow.status = EscrowStatus::Disputed;
        escrow.evidence_hash = evidence_hash.clone();
        Self::save_escrow(&env, escrow_id, &escrow);

        env.events().publish(
            (Symbol::new(&env, "escrow_disputed"), escrow_id, caller),
            evidence_hash,
        );
    }

    pub fn resolve_dispute(
        env: Env,
        escrow_id: u64,
        arbitrator: Address,
        release_to_recipient: bool,
        evidence_hash: Vec<u8>,
    ) {
        arbitrator.require_auth();
        let escrow = Self::load_escrow(&env, escrow_id);
        if escrow.status != EscrowStatus::Disputed {
            panic!("Escrow not in dispute");
        }
        if arbitrator != escrow.arbitrator {
            panic!("Not authorized to resolve dispute");
        }

        let receiver = if release_to_recipient {
            escrow.recipient.clone()
        } else {
            escrow.sender.clone()
        };
        Self::finalize_settlement(env, escrow_id, escrow, receiver, !release_to_recipient, true, evidence_hash);
    }

    pub fn release_funds(env: Env, escrow_id: u64, caller: Address) {
        caller.require_auth();
        let escrow = Self::load_escrow(&env, escrow_id);
        if escrow.status != EscrowStatus::Active && escrow.status != EscrowStatus::Disputed {
            panic!("Escrow not active or disputed");
        }
        if caller != escrow.arbitrator {
            panic!("Not authorized to release");
        }

        Self::finalize_settlement(env, escrow_id, escrow, escrow.recipient.clone(), false, true, Vec::new(&env));
    }

    pub fn cancel_escrow(env: Env, escrow_id: u64, caller: Address) {
        caller.require_auth();
        let escrow = Self::load_escrow(&env, escrow_id);
        if escrow.status != EscrowStatus::Active && escrow.status != EscrowStatus::Disputed {
            panic!("Escrow not active or disputed");
        }

        let now = env.ledger().timestamp();
        let is_arbitrator = caller == escrow.arbitrator;
        let is_expired_refund = caller == escrow.sender && now >= escrow.unlock_time;

        if !is_arbitrator && !is_expired_refund {
            panic!("Not authorized to cancel");
        }

        let apply_arbitrator_fee = is_arbitrator;
        Self::finalize_settlement(env, escrow_id, escrow, escrow.sender.clone(), true, apply_arbitrator_fee, escrow.evidence_hash.clone());
    }

    pub fn auto_resolve(env: Env, escrow_id: u64) {
        let escrow = Self::load_escrow(&env, escrow_id);
        if escrow.status != EscrowStatus::Active && escrow.status != EscrowStatus::Disputed {
            panic!("Escrow cannot be auto-resolved");
        }
        let now = env.ledger().timestamp();
        if now < escrow.unlock_time {
            panic!("Unlock time has not passed");
        }

        Self::finalize_settlement(env, escrow_id, escrow, escrow.sender.clone(), true, false, escrow.evidence_hash.clone());
    }

    pub fn get_escrow(env: Env, id: u64) -> EscrowInfo {
        Self::load_escrow(&env, id)
    }

    fn assert_fee_config(platform_fee_bps: u32, arbitrator_fee_bps: u32) {
        if platform_fee_bps > 10_000 || arbitrator_fee_bps > 10_000 {
            panic!("Invalid fee basis points");
        }
        if platform_fee_bps + arbitrator_fee_bps > 10_000 {
            panic!("Fee basis points exceed 100% of amount");
        }
    }

    fn get_platform_config(env: &Env) -> (Address, u32, u32) {
        let platform_account: Address = env
            .storage()
            .instance()
            .get(&DataKey::PlatformFeeAccount)
            .unwrap_or(env.current_contract_address());
        let platform_fee_bps: u32 = env
            .storage()
            .instance()
            .get(&DataKey::PlatformFeeBps)
            .unwrap_or(0);
        let arbitrator_fee_bps: u32 = env
            .storage()
            .instance()
            .get(&DataKey::ArbitratorFeeBps)
            .unwrap_or(0);
        (platform_account, platform_fee_bps, arbitrator_fee_bps)
    }

    fn compute_fees(amount: i128, platform_fee_bps: u32, arbitrator_fee_bps: u32) -> (i128, i128, i128) {
        let platform_fee = amount * i128::from(platform_fee_bps) / 10_000;
        let arbitrator_fee = amount * i128::from(arbitrator_fee_bps) / 10_000;
        let net_amount = amount - platform_fee - arbitrator_fee;
        if net_amount < 0 {
            panic!("Fee configuration invalid");
        }
        (platform_fee, arbitrator_fee, net_amount)
    }

    fn load_escrow(env: &Env, escrow_id: u64) -> EscrowInfo {
        env.storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .expect("Escrow not found")
    }

    fn save_escrow(env: &Env, escrow_id: u64, escrow: &EscrowInfo) {
        env.storage().persistent().set(&DataKey::Escrow(escrow_id), escrow);
    }

    fn finalize_settlement(
        env: Env,
        escrow_id: u64,
        mut escrow: EscrowInfo,
        receiver: Address,
        is_refund: bool,
        apply_arbitrator_fee: bool,
        evidence_hash: Vec<u8>,
    ) {
        let (platform_account, platform_fee_bps, arbitrator_fee_bps) = Self::get_platform_config(&env);
        let arbitrator_fee_bps = if apply_arbitrator_fee { arbitrator_fee_bps } else { 0 };
        let (platform_fee, arbitrator_fee, net_amount) = Self::compute_fees(escrow.amount, platform_fee_bps, arbitrator_fee_bps);

        let token_client = token::Client::new(&env, &escrow.token);
        if platform_fee > 0 {
            token_client.transfer(&env.current_contract_address(), &platform_account, &platform_fee);
        }
        if arbitrator_fee > 0 {
            token_client.transfer(&env.current_contract_address(), &escrow.arbitrator, &arbitrator_fee);
        }
        if net_amount > 0 {
            token_client.transfer(&env.current_contract_address(), &receiver, &net_amount);
        }

        escrow.status = if is_refund {
            EscrowStatus::Cancelled
        } else {
            EscrowStatus::Released
        };
        escrow.evidence_hash = evidence_hash.clone();
        Self::save_escrow(&env, escrow_id, &escrow);

        let decision = if is_refund {
            Symbol::new(&env, "sender")
        } else {
            Symbol::new(&env, "recipient")
        };

        env.events().publish(
            (
                Symbol::new(&env, "escrow_resolved"),
                escrow_id,
                receiver,
                escrow.sender.clone(),
                escrow.recipient.clone(),
                escrow.arbitrator.clone(),
            ),
            (decision, evidence_hash, platform_fee, arbitrator_fee, net_amount),
        );
    }
}

mod test;
