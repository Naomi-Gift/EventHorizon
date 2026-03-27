#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, Symbol};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    TokenA,
    TokenB,
    ReserveA,
    ReserveB,
    LPSupply,
    LPBalance(Address),
}

#[contract]
pub struct ConstantProductAMM;

#[contractimpl]
impl ConstantProductAMM {
    /// Initializes the AMM with two tokens.
    pub fn initialize(env: Env, token_a: Address, token_b: Address) {
        if env.storage().instance().has(&DataKey::TokenA) {
            panic!("Already initialized");
        }
        // Ensure token_a < token_b for canonical pair
        let (a, b) = if token_a < token_b {
            (token_a, token_b)
        } else {
            (token_b, token_a)
        };

        env.storage().instance().set(&DataKey::TokenA, &a);
        env.storage().instance().set(&DataKey::TokenB, &b);
        env.storage().instance().set(&DataKey::ReserveA, &0i128);
        env.storage().instance().set(&DataKey::ReserveB, &0i128);
        env.storage().instance().set(&DataKey::LPSupply, &0i128);
    }

    /// Deposits tokens and mints LP tokens representing the share of the pool.
    #[allow(deprecated)]
    pub fn add_liquidity(
        env: Env,
        to: Address,
        amount_a_desired: i128,
        amount_b_desired: i128,
    ) -> i128 {
        to.require_auth();

        let token_a: Address = env
            .storage()
            .instance()
            .get(&DataKey::TokenA)
            .expect("Not init");
        let token_b: Address = env
            .storage()
            .instance()
            .get(&DataKey::TokenB)
            .expect("Not init");
        let reserve_a: i128 = env
            .storage()
            .instance()
            .get(&DataKey::ReserveA)
            .unwrap_or(0);
        let reserve_b: i128 = env
            .storage()
            .instance()
            .get(&DataKey::ReserveB)
            .unwrap_or(0);
        let total_supply: i128 = env
            .storage()
            .instance()
            .get(&DataKey::LPSupply)
            .unwrap_or(0);

        let (amount_a, amount_b, lp_to_mint) = if total_supply == 0 {
            let amount_lp = Self::sqrt(amount_a_desired * amount_b_desired);
            (amount_a_desired, amount_b_desired, amount_lp)
        } else {
            // Optimal B = Desired A * Reserve B / Reserve A
            let amount_b_optimal = (amount_a_desired * reserve_b) / reserve_a;
            if amount_b_optimal <= amount_b_desired {
                let amount_lp = (amount_a_desired * total_supply) / reserve_a;
                (amount_a_desired, amount_b_optimal, amount_lp)
            } else {
                let amount_a_optimal = (amount_b_desired * reserve_a) / reserve_b;
                let amount_lp = (amount_b_desired * total_supply) / reserve_b;
                (amount_a_optimal, amount_b_desired, amount_lp)
            }
        };

        // Transfer tokens to the AMM
        token::Client::new(&env, &token_a).transfer(
            &to,
            &env.current_contract_address(),
            &amount_a,
        );
        token::Client::new(&env, &token_b).transfer(
            &to,
            &env.current_contract_address(),
            &amount_b,
        );

        // Update reserves and supply
        env.storage()
            .instance()
            .set(&DataKey::ReserveA, &(reserve_a + amount_a));
        env.storage()
            .instance()
            .set(&DataKey::ReserveB, &(reserve_b + amount_b));
        env.storage()
            .instance()
            .set(&DataKey::LPSupply, &(total_supply + lp_to_mint));

        // Update LP balance
        let balance: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::LPBalance(to.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::LPBalance(to.clone()), &(balance + lp_to_mint));

        env.events().publish(
            (Symbol::new(&env, "liquidity_added"), to),
            (amount_a, amount_b, lp_to_mint),
        );

        lp_to_mint
    }

    /// Burns LP tokens and returns the underlying assets.
    #[allow(deprecated)]
    pub fn remove_liquidity(env: Env, from: Address, lp_amount: i128) -> (i128, i128) {
        from.require_auth();

        let balance: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::LPBalance(from.clone()))
            .unwrap_or(0);
        if balance < lp_amount {
            panic!("Insufficient LP balance");
        }

        let token_a: Address = env
            .storage()
            .instance()
            .get(&DataKey::TokenA)
            .expect("Not init");
        let token_b: Address = env
            .storage()
            .instance()
            .get(&DataKey::TokenB)
            .expect("Not init");
        let reserve_a: i128 = env
            .storage()
            .instance()
            .get(&DataKey::ReserveA)
            .unwrap_or(0);
        let reserve_b: i128 = env
            .storage()
            .instance()
            .get(&DataKey::ReserveB)
            .unwrap_or(0);
        let total_supply: i128 = env
            .storage()
            .instance()
            .get(&DataKey::LPSupply)
            .unwrap_or(0);

        let amount_a = (lp_amount * reserve_a) / total_supply;
        let amount_b = (lp_amount * reserve_b) / total_supply;

        // Transfers back to user
        token::Client::new(&env, &token_a).transfer(
            &env.current_contract_address(),
            &from,
            &amount_a,
        );
        token::Client::new(&env, &token_b).transfer(
            &env.current_contract_address(),
            &from,
            &amount_b,
        );

        // Update state
        env.storage()
            .instance()
            .set(&DataKey::ReserveA, &(reserve_a - amount_a));
        env.storage()
            .instance()
            .set(&DataKey::ReserveB, &(reserve_b - amount_b));
        env.storage()
            .instance()
            .set(&DataKey::LPSupply, &(total_supply - lp_amount));
        env.storage()
            .persistent()
            .set(&DataKey::LPBalance(from.clone()), &(balance - lp_amount));

        env.events().publish(
            (Symbol::new(&env, "liquidity_removed"), from),
            (amount_a, amount_b, lp_amount),
        );

        (amount_a, amount_b)
    }

    /// Swaps `token_in` for another token. Uses constant product (x*y=k).
    #[allow(deprecated)]
    pub fn swap(
        env: Env,
        from: Address,
        token_in: Address,
        amount_in: i128,
        min_amount_out: i128,
    ) -> i128 {
        from.require_auth();

        let token_a_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::TokenA)
            .expect("Not init");
        let token_b_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::TokenB)
            .expect("Not init");
        let reserve_a: i128 = env
            .storage()
            .instance()
            .get(&DataKey::ReserveA)
            .unwrap_or(0);
        let reserve_b: i128 = env
            .storage()
            .instance()
            .get(&DataKey::ReserveB)
            .unwrap_or(0);

        let (reserve_in, reserve_out, is_a_in) = if token_in == token_a_addr {
            (reserve_a, reserve_b, true)
        } else if token_in == token_b_addr {
            (reserve_b, reserve_a, false)
        } else {
            panic!("Invalid token");
        };

        // Formula: dy = y * (dx * 997) / (x * 1000 + dx * 997)
        let amount_in_with_fee = amount_in * 997;
        let numerator = amount_in_with_fee * reserve_out;
        let denominator = (reserve_in * 1000) + amount_in_with_fee;
        let amount_out = numerator / denominator;

        if amount_out < min_amount_out {
            panic!("Slippage limit exceeded");
        }

        // Transfer funds
        let token_out_addr = if is_a_in {
            &token_b_addr
        } else {
            &token_a_addr
        };
        token::Client::new(&env, &token_in).transfer(
            &from,
            &env.current_contract_address(),
            &amount_in,
        );
        token::Client::new(&env, token_out_addr).transfer(
            &env.current_contract_address(),
            &from,
            &amount_out,
        );

        // Update reserves
        if is_a_in {
            env.storage()
                .instance()
                .set(&DataKey::ReserveA, &(reserve_a + amount_in));
            env.storage()
                .instance()
                .set(&DataKey::ReserveB, &(reserve_b - amount_out));
        } else {
            env.storage()
                .instance()
                .set(&DataKey::ReserveA, &(reserve_a - amount_out));
            env.storage()
                .instance()
                .set(&DataKey::ReserveB, &(reserve_b + amount_in));
        }

        env.events().publish(
            (Symbol::new(&env, "swap"), from),
            (token_in, amount_in, amount_out),
        );

        amount_out
    }

    /// Returns pool reserves and total supply.
    pub fn get_pool_info(env: Env) -> (i128, i128, i128) {
        let ra = env
            .storage()
            .instance()
            .get(&DataKey::ReserveA)
            .unwrap_or(0);
        let rb = env
            .storage()
            .instance()
            .get(&DataKey::ReserveB)
            .unwrap_or(0);
        let supply = env
            .storage()
            .instance()
            .get(&DataKey::LPSupply)
            .unwrap_or(0);
        (ra, rb, supply)
    }

    /// Newton-Raphson square root for i128.
    fn sqrt(y: i128) -> i128 {
        if y < 0 {
            panic!("Negative sqrt");
        }
        if y < 4 {
            if y == 0 {
                return 0;
            }
            return 1;
        }
        let mut z = y;
        let mut x = y / 2 + 1;
        while x < z {
            z = x;
            x = (y / x + x) / 2;
        }
        z
    }
}

mod test;
