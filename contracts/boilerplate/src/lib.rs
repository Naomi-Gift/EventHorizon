#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Env, Symbol};

#[contract]
pub struct EventContract;

#[contractimpl]
impl EventContract {
    /// Emit a test event for EventHorizon to catch
    #[allow(deprecated)]
    pub fn trigger_event(env: Env, value: Symbol) {
        // Emit an event with the topic "test_event" and the provided value
        env.events().publish((symbol_short!("test_evt"),), value);
    }
}

mod test;
