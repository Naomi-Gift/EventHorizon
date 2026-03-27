# Pull Request: Implement Recurring Subscription Payment Contract

## Description
This PR implements a Soroban smart contract for handling automated recurring payments between users (subscribers) and services (providers).

### Key Features:
- **Approval-based Withdrawal**: Utilizes the standard `token.transfer_from` mechanism for secure automated payments.
- **Billing Cycle Management**: Tracks the `last_payment` timestamp and `frequency` to ensure payments are only processed at the correct intervals.
- **Lifecycle Logic**: Includes functions for users to `pause`, `resume`, or `cancel` their subscriptions with proper authorization.
- **Event-Driven**: Emits `payment_processed` events for every successful transaction for EventHorizon to catch and trigger downstream Web2 actions.

## Changes:
- [NEW] `contracts/subscriptions/`: Core contract logic and test suite.
- [MODIFY] `contracts/Cargo.toml`: Added `subscriptions` to the workspace members.

## Testing:
- Implemented a comprehensive unit test suite in `src/test.rs` covering subscription creation, recurring payment cycles, lifecycle transitions (pause/resume/cancel), and error handling for premature payments.

#23
