#![no_std]
//! Immutable, non-custodial setup: one invocation registers a mandate and
//! approves its exact token allowance. No payment, administrator or upgrade API.
use soroban_sdk::{
    contract, contractclient, contractimpl, symbol_short, token, Address, BytesN, Env,
};

#[contractclient(name = "RegistryClient")]
pub trait Registry {
    #[allow(clippy::too_many_arguments)]
    fn register_mandate(
        env: Env,
        user: Address,
        agent: Address,
        merchant: Address,
        asset: Address,
        max_amount: i128,
        expiry: u64,
        vc_hash: BytesN<32>,
    ) -> BytesN<32>;
}

#[contract]
pub struct WalletSetup;

#[contractimpl]
impl WalletSetup {
    /// Fixed at deployment; cannot be redirected or upgraded later.
    pub fn __constructor(env: Env, registry: Address, asset: Address) {
        env.storage()
            .instance()
            .set(&symbol_short!("registry"), &registry);
        env.storage()
            .instance()
            .set(&symbol_short!("asset"), &asset);
    }

    pub fn get_config(env: Env) -> (Address, Address) {
        (
            env.storage()
                .instance()
                .get(&symbol_short!("registry"))
                .unwrap(),
            env.storage()
                .instance()
                .get(&symbol_short!("asset"))
                .unwrap(),
        )
    }

    /// Authorize the complete setup once. Both child calls must succeed or all
    /// state changes revert. The registry, never this helper or agent, spends.
    #[allow(clippy::too_many_arguments)]
    pub fn register_and_approve(
        env: Env,
        user: Address,
        agent: Address,
        merchant: Address,
        max_amount: i128,
        expiry: u64,
        vc_hash: BytesN<32>,
        allowance_expiration: u32,
    ) -> BytesN<32> {
        user.require_auth();
        assert!(max_amount > 0, "positive spending cap required");
        assert!(
            expiry > env.ledger().timestamp(),
            "future mandate expiry required"
        );
        let ledger = env.ledger().sequence();
        assert!(
            allowance_expiration > ledger && allowance_expiration <= ledger.saturating_add(17_280),
            "allowance must expire within 17280 ledgers"
        );
        let (registry, asset) = Self::get_config(env.clone());
        let id = RegistryClient::new(&env, &registry).register_mandate(
            &user,
            &agent,
            &merchant,
            &asset,
            &max_amount,
            &expiry,
            &vc_hash,
        );
        token::TokenClient::new(&env, &asset).approve(
            &user,
            &registry,
            &max_amount,
            &allowance_expiration,
        );
        id
    }
}

#[cfg(test)]
mod test;
