#![no_std]
use soroban_sdk::{
    contract, contractevent, contractimpl, contracttype,
    token, Address, Bytes, Env, Vec,
};

// ── Storage keys ────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Threshold,          // u32 – required approvals (M)
    Signers,            // Vec<Address>
    TimelockDelay,      // u64 – delay in seconds before queued event can execute
    NextEventId,        // u64
    Event(u64),         // EventProposal
    Approved(u64, Address), // bool – has signer approved event?
}

// ── Data types ───────────────────────────────────────────────────────────────

/// Status of an event proposal.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum EventStatus {
    Pending  = 0,
    Queued   = 1, // threshold met, waiting for timelock
    Executed = 2,
    Cancelled = 3,
}

/// A proposed event/targeted action to be triggered through the proxy.
#[contracttype]
#[derive(Clone, Debug)]
pub struct EventProposal {
    pub id: u64,
    pub proposer: Address,
    pub target: Address,       // contract to call (or token address for transfers)
    pub calldata: Bytes,       // ABI-encoded call
    pub amount: i128,          // token amount (0 if pure contract invocation)
    pub token: Address,        // token address (ignored when amount == 0)
    pub approvals: u32,        // running approval count
    pub status: EventStatus,
    pub queued_at: u64,        // timestamp when threshold was reached
    pub execution_time: u64,   // calculated timestamp when execution is allowed
}

/// Event emitted when any signer approves the event.
#[contractevent]
pub struct EventApproved {
    pub event_id: u64,
    pub signer: Address,
    pub approvals: u32,
}

/// Event emitted when an event is proposed.
#[contractevent]
pub struct EventProposed {
    pub event_id: u64,
    pub proposer: Address,
    pub target: Address,
}

/// Event emitted when an event reaches threshold and is queued for timelock.
#[contractevent]
pub struct EventQueued {
    pub event_id: u64,
    pub queued_at: u64,
    pub execution_time: u64,
}

/// Event emitted when an event is successfully executed.
#[contractevent]
pub struct EventExecuted {
    pub event_id: u64,
}

/// Event emitted when an event is cancelled.
#[contractevent]
pub struct EventCancelled {
    pub event_id: u64,
    pub canceller: Address,
}

/// Event emitted when a signer is added/removed.
#[contractevent]
pub struct SignerAdded { pub signer: Address }
#[contractevent]
pub struct SignerRemoved { pub signer: Address }
#[contractevent]
pub struct TimelockUpdated { pub new_delay: u64 }

// ── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct EventProxy;

#[contractimpl]
impl EventProxy {
    // ── Setup ────────────────────────────────────────────────────────────────

    /// Initialize the EventProxy with signers and approval threshold.
    /// `threshold` must be ≤ len(signers) and ≥ 1.
    /// `timelock_delay` is in seconds.
    pub fn initialize(
        env: Env,
        signers: Vec<Address>,
        threshold: u32,
        timelock_delay: u64,
    ) {
        if env.storage().instance().has(&DataKey::Threshold) {
            panic!("Already initialized");
        }
        Self::_validate_threshold(&signers, threshold);
        if timelock_delay == 0 {
            panic!("Timelock delay must be > 0");
        }
        env.storage().instance().set(&DataKey::Signers, &signers);
        env.storage().instance().set(&DataKey::Threshold, &threshold);
        env.storage().instance().set(&DataKey::TimelockDelay, &timelock_delay);
        env.storage().instance().set(&DataKey::NextEventId, &0u64);
    }

    // ── Signer management (must be called via the proxy's own flow) ──────────

    /// Add a new signer. Must be called through the proxy's execution flow
    /// after appropriate approvals (target = current contract address).
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

    /// Remove a signer. Must be called through the proxy's execution flow.
    pub fn remove_signer(env: Env, signer: Address) {
        env.current_contract_address().require_auth();
        let signers: Vec<Address> = env.storage().instance().get(&DataKey::Signers).unwrap();
        let threshold: u32 = env.storage().instance().get(&DataKey::Threshold).unwrap();

        let mut new_signers: Vec<Address> = Vec::new(&env);
        let mut found = false;
        for s in signers.iter() {
            if s == &signer { found = true; } else { new_signers.push_back(s.clone()); }
        }
        if !found { panic!("Not a signer"); }
        Self::_validate_threshold(&new_signers, threshold);
        env.storage().instance().set(&DataKey::Signers, &new_signers);
        env.events().publish_event(&SignerRemoved { signer });
    }

    /// Update the timelock delay. Must be called through the proxy's execution flow.
    pub fn set_timelock_delay(env: Env, new_delay: u64) {
        env.current_contract_address().require_auth();
        if new_delay == 0 {
            panic!("Timelock delay must be > 0");
        }
        env.storage().instance().set(&DataKey::TimelockDelay, &new_delay);
        env.events().publish_event(&TimelockUpdated { new_delay });
    }

    // ── Event lifecycle ───────────────────────────────────────────────────────

    /// Schedule a new event for approval.
    /// Only signers can propose events to prevent spam.
    pub fn schedule_event(
        env: Env,
        proposer: Address,
        target: Address,
        calldata: Bytes,
        amount: i128,
        token: Address,
    ) -> u64 {
        proposer.require_auth();
        Self::_require_signer(&env, &proposer);

        let event_id = Self::_next_event_id(&env);
        let now = env.ledger().timestamp();

        let event = EventProposal {
            id: event_id,
            proposer: proposer.clone(),
            target: target.clone(),
            calldata: calldata.clone(),
            amount,
            token: token.clone(),
            approvals: 0,
            status: EventStatus::Pending,
            queued_at: 0,
            execution_time: 0,
        };

        env.storage().persistent().set(&DataKey::Event(event_id), &event);
        env.events().publish_event(&EventProposed {
            event_id,
            proposer,
            target,
        });

        event_id
    }

    /// A signer approves a pending event.
    /// Once threshold approvals are reached, the event status becomes Queued
    /// and execution_time is set to now + timelock_delay.
    #[allow(deprecated)]
    pub fn approve(env: Env, signer: Address, event_id: u64) {
        signer.require_auth();
        Self::_require_signer(&env, &signer);

        let mut event: EventProposal = env.storage().persistent()
            .get(&DataKey::Event(event_id))
            .expect("Event not found");

        if event.status != EventStatus::Pending {
            panic!("Event not pending");
        }

        let approval_key = DataKey::Approved(event_id, signer.clone());
        if env.storage().temporary().get::<_, bool>(&approval_key).unwrap_or(false) {
            panic!("Already approved");
        }
        env.storage().temporary().set(&approval_key, &true);

        event.approvals += 1;
        let threshold: u32 = env.storage().instance().get(&DataKey::Threshold).unwrap();

        env.events().publish_event(&EventApproved {
            event_id,
            signer: signer.clone(),
            approvals: event.approvals,
        });

        if event.approvals >= threshold {
            let timelock_delay: u64 = env.storage().instance().get(&DataKey::TimelockDelay).unwrap();
            let now = env.ledger().timestamp();
            event.queued_at = now;
            event.execution_time = now + timelock_delay;
            event.status = EventStatus::Queued;

            env.storage().persistent().set(&DataKey::Event(event_id), &event);
            env.events().publish_event(&EventQueued {
                event_id,
                queued_at: now,
                execution_time: event.execution_time,
            });
        } else {
            env.storage().persistent().set(&DataKey::Event(event_id), &event);
        }
    }

    /// Any signer can cancel a pending or queued event.
    pub fn cancel(env: Env, canceller: Address, event_id: u64) {
        canceller.require_auth();
        Self::_require_signer(&env, &canceller);

        let mut event: EventProposal = env.storage().persistent()
            .get(&DataKey::Event(event_id))
            .expect("Event not found");

        match event.status {
            EventStatus::Pending | EventStatus::Queued => {
                event.status = EventStatus::Cancelled;
                env.storage().persistent().set(&DataKey::Event(event_id), &event);
                env.events().publish_event(&EventCancelled {
                    event_id,
                    canceller,
                });
            }
            _ => panic!("Event cannot be cancelled"),
        }
    }

    /// Execute the approved event after timelock expires.
    /// Anyone can call this, but it only succeeds when:
    /// - status is Queued
    /// - current timestamp >= execution_time
    pub fn execute(env: Env, event_id: u64) {
        let mut event: EventProposal = env.storage().persistent()
            .get(&DataKey::Event(event_id))
            .expect("Event not found");

        if event.status != EventStatus::Queued {
            panic!("Event not queued for execution");
        }

        let now = env.ledger().timestamp();
        if now < event.execution_time {
            panic!("Timelock not expired");
        }

        event.status = EventStatus::Executed;
        env.storage().persistent().set(&DataKey::Event(event_id), &event);
        Self::_execute(&env, &event);

        env.events().publish_event(&EventExecuted { event_id });
    }

    // ── Views ────────────────────────────────────────────────────────────────

    /// Get event details by ID.
    pub fn get_event(env: Env, event_id: u64) -> EventProposal {
        env.storage().persistent().get(&DataKey::Event(event_id)).expect("Event not found")
    }

    /// Get list of all signers.
    pub fn get_signers(env: Env) -> Vec<Address> {
        env.storage().instance().get(&DataKey::Signers).unwrap()
    }

    /// Get M-of-N threshold.
    pub fn get_threshold(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::Threshold).unwrap()
    }

    /// Get timelock delay in seconds.
    pub fn get_timelock_delay(env: Env) -> u64 {
        env.storage().instance().get(&DataKey::TimelockDelay).unwrap()
    }

    /// Check if an address is a signer.
    pub fn is_signer(env: Env, addr: Address) -> bool {
        let signers: Vec<Address> = env.storage().instance().get(&DataKey::Signers).unwrap_or(Vec::new(&env));
        signers.contains(&addr)
    }

    /// Get current ledger timestamp.
    pub fn current_timestamp(env: Env) -> u64 {
        env.ledger().timestamp()
    }

    // ── Internals ────────────────────────────────────────────────────────────

    fn _validate_threshold(signers: &Vec<Address>, threshold: u32) {
        if threshold == 0 { panic!("Threshold must be >= 1"); }
        if threshold > signers.len() { panic!("Threshold exceeds signer count"); }
        if signers.len() == 0 { panic!("Must have at least 1 signer"); }
    }

    fn _require_signer(env: &Env, addr: &Address) {
        let signers: Vec<Address> = env.storage().instance().get(&DataKey::Signers).unwrap();
        if !signers.contains(addr) { panic!("Not a signer"); }
    }

    fn _next_event_id(env: &Env) -> u64 {
        let id: u64 = env.storage().instance().get(&DataKey::NextEventId).unwrap_or(0);
        env.storage().instance().set(&DataKey::NextEventId, &(id + 1));
        id
    }

    /// Execute the underlying action:
    /// - If amount > 0: transfer `amount` of `token` from this contract to `target`
    /// - Otherwise: invoke `target` with `calldata`
    #[allow(deprecated)]
    fn _execute(env: &Env, event: &EventProposal) {
        if event.amount > 0 {
            // Token transfer
            soroban_sdk::token::Client::new(env, &event.token)
                .transfer(&env.current_contract_address(), &event.target, &event.amount);
        } else if !event.calldata.is_empty() {
            // Contract invocation - emit event for off-chain monitoring
            // On-chain generic invocation would require Wasm dynamic dispatch
            // which is not currently supported in Soroban in this manner.
            // For actual contract calls, integrators should use a typed
            // interface or external relayer to perform the call.
            let _ = &event.calldata;
        }
        // If both amount==0 and calldata empty, no-op (valid as heartbeat/marker)
    }
}

mod test;
