#![no_std]
use soroban_sdk::{
    contract, contractimpl, contractevent, vec, Address, Env, Error, InvokeError, Symbol, Val, Vec,
};

/// Result of a single simulation call.
#[soroban_sdk::contracttype]
pub struct SimResult {
    /// Caller-supplied identifier, echoed back.
    pub call_id: u32,
    /// Whether the inner call succeeded.
    pub success: bool,
}

/// Event emitted after every simulated call.
#[contractevent]
pub struct SimResultEvent {
    /// Matches the `call_id` supplied to `simulate`.
    pub call_id: u32,
    /// `true` if the inner call succeeded, `false` if it panicked/errored.
    pub success: bool,
}

#[contract]
pub struct GasEstimator;

#[contractimpl]
impl GasEstimator {
    /// Simulate a single cross-contract call.
    ///
    /// Uses `try_invoke_contract` so this function **never reverts** regardless
    /// of what the target does. A `SimResultEvent` is always emitted so that
    /// EventHorizon (and Soroban RPC `simulateTransaction`) can capture it.
    ///
    /// Actual resource usage (CPU instructions, memory bytes) is returned by
    /// the RPC layer in the simulation response; the emitted event lets callers
    /// correlate success/failure with those host-reported numbers.
    pub fn simulate(
        env: Env,
        call_id: u32,
        contract: Address,
        func: Symbol,
        args: Vec<Val>,
    ) -> SimResult {
        let outcome: Result<Result<Val, _>, Result<Error, InvokeError>> =
            env.try_invoke_contract::<Val, Error>(&contract, &func, args);

        let success = matches!(outcome, Ok(Ok(_)));

        SimResultEvent { call_id, success }.publish(&env);

        SimResult { call_id, success }
    }

    /// Batch-simulate multiple calls.
    ///
    /// Each entry: `(call_id, contract_address, func_name, args)`.
    /// Individual failures do **not** abort the batch.
    pub fn simulate_batch(
        env: Env,
        calls: Vec<(u32, Address, Symbol, Vec<Val>)>,
    ) -> Vec<SimResult> {
        let mut results: Vec<SimResult> = vec![&env];
        for call in calls.iter() {
            let (call_id, contract, func, args) = call;
            results.push_back(Self::simulate(env.clone(), call_id, contract, func, args));
        }
        results
    }
}

mod test;
