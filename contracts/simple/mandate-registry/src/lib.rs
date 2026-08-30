#![no_std]
//! MandateRegistry — ACKRATE's on-chain enforcement layer.
//!
//! The contract is the entire protocol and is small by design: a small
//! interface is reviewable. Money moves only through `execute_payment`, which
//! validates-and-consumes the mandate atomically before transferring. The SDK
//! is untrusted; this contract is the source of truth.
//!
mod storage;

use soroban_sdk::token::TokenClient;
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
};

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Error {
    AlreadyExists = 1,
    NotFound = 2,
    MandateExpired = 4,
    MandateRevoked = 5,
    BudgetExceeded = 6,
    MerchantOutOfScope = 7,
    BadSequence = 8,
    InvalidAmount = 9,
    Paused = 10,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Mandate {
    pub user: Address,
    pub agent: Address,
    pub merchant: Address,
    pub asset: Address,
    pub max_amount: i128,
    pub spent: i128,
    pub expiry: u64,
    pub seq: u32,
    pub status: Status,
    pub vc_hash: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Active,
    Revoked,
    Exhausted,
}

#[contract]
pub struct MandateRegistry;

#[contractimpl]
impl MandateRegistry {
    /// Atomically establishes the initial administrator during deployment.
    /// Constructors run only once; WASM upgrades do not run them again.
    pub fn __constructor(env: Env, admin: Address) {
        storage::set_admin(&env, &admin);
        storage::set_paused(&env, false);
    }

    /// Current operational administrator.
    pub fn get_admin(env: Env) -> Address {
        storage::get_admin(&env)
    }

    /// Rotate operational authority. Authorized by the current administrator.
    pub fn set_admin(env: Env, new_admin: Address) {
        let current = storage::get_admin(&env);
        current.require_auth();
        storage::set_admin(&env, &new_admin);
        env.events().publish((symbol_short!("admin"),), new_admin);
    }

    /// Emergency stop for the sole money-moving path.
    pub fn pause(env: Env) {
        let admin = storage::get_admin(&env);
        admin.require_auth();
        if !storage::is_paused(&env) {
            storage::set_paused(&env, true);
            env.events().publish((symbol_short!("paused"), admin), ());
        }
    }

    /// Restore the money-moving path after an emergency stop.
    pub fn unpause(env: Env) {
        let admin = storage::get_admin(&env);
        admin.require_auth();
        if storage::is_paused(&env) {
            storage::set_paused(&env, false);
            env.events().publish((symbol_short!("unpaused"), admin), ());
        }
    }

    /// Read the emergency-stop state without authorization.
    pub fn is_paused(env: Env) -> bool {
        storage::is_paused(&env)
    }

    /// Replace this contract's WASM at the same address. The current admin is
    /// the sole authority; account thresholds provide any desired multisig.
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) {
        let admin = storage::get_admin(&env);
        admin.require_auth();
        env.events()
            .publish((symbol_short!("upgrade"), admin), new_wasm_hash.clone());
        env.deployer().update_current_contract_wasm(new_wasm_hash);
    }

    /// Store a user-signed mandate from its authorized parameters. The contract
    /// sets `spent=0, seq=0, status=Active` itself. Authorized by `user`.
    /// Returns the mandate id (= `vc_hash`, the storage key).
    #[allow(clippy::too_many_arguments)]
    pub fn register_mandate(
        env: Env,
        user: Address,
        agent: Address,
        merchant: Address,
        asset: Address,
        max_amount: i128,
        expiry: u64,
        vc_hash: BytesN<32>,
    ) -> Result<BytesN<32>, Error> {
        user.require_auth();
        if max_amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        if expiry <= env.ledger().timestamp() {
            return Err(Error::MandateExpired);
        }
        if storage::has_mandate(&env, &vc_hash) {
            return Err(Error::AlreadyExists);
        }
        let mandate = Mandate {
            user: user.clone(),
            agent,
            merchant,
            asset,
            max_amount,
            spent: 0,
            expiry,
            seq: 0,
            status: Status::Active,
            vc_hash: vc_hash.clone(),
        };
        storage::set_mandate(&env, &vc_hash, &mandate);
        env.events()
            .publish((symbol_short!("register"), user), vc_hash.clone());
        Ok(vc_hash)
    }

    /// Read-only preflight — would this spend be permitted right now? Mutates
    /// nothing and requires no auth; the authoritative consume happens only in
    /// `execute_payment`. (It is a dry-run; it consumes nothing.)
    pub fn validate_mandate(
        env: Env,
        mandate_id: BytesN<32>,
        amount: i128,
        merchant: Address,
    ) -> Result<(), Error> {
        let mandate = storage::get_mandate(&env, mandate_id)?;
        check_mandate(&env, &mandate, amount, &merchant)
    }

    /// The only money path. Atomic: require_auth(agent) → replay guard
    /// (`expected_seq` == current `seq`, else `BadSequence`) → re-validate →
    /// advance spent+seq → SEP-41 transfer_from(user → merchant). Reverts on any
    /// failure. `expected_seq` is the mandate's current sequence (read from
    /// `get_mandate`), preventing duplicate/out-of-order consumption.
    pub fn execute_payment(
        env: Env,
        mandate_id: BytesN<32>,
        amount: i128,
        expected_seq: u32,
    ) -> Result<(), Error> {
        if storage::is_paused(&env) {
            return Err(Error::Paused);
        }
        let mut mandate = storage::get_mandate(&env, mandate_id.clone())?;
        mandate.agent.require_auth();
        if expected_seq != mandate.seq {
            return Err(Error::BadSequence);
        }
        let merchant = mandate.merchant.clone();
        check_mandate(&env, &mandate, amount, &merchant)?;
        mandate.spent += amount;
        mandate.seq += 1;
        if mandate.spent == mandate.max_amount {
            mandate.status = Status::Exhausted;
        }
        storage::set_mandate(&env, &mandate_id, &mandate);
        TokenClient::new(&env, &mandate.asset).transfer_from(
            &env.current_contract_address(),
            &mandate.user,
            &merchant,
            &amount,
        );
        env.events()
            .publish((symbol_short!("payment"), merchant), (mandate_id, amount));
        Ok(())
    }

    /// User withdraws consent; marks the mandate Revoked. Authorized by the user.
    pub fn revoke_mandate(env: Env, mandate_id: BytesN<32>) -> Result<(), Error> {
        let mut mandate = storage::get_mandate(&env, mandate_id.clone())?;
        mandate.user.require_auth();
        mandate.status = Status::Revoked;
        storage::set_mandate(&env, &mandate_id, &mandate);
        env.events().publish((symbol_short!("revoke"),), mandate_id);
        Ok(())
    }

    /// Read-only accessor for the stored mandate (inspection / preflight).
    pub fn get_mandate(env: Env, mandate_id: BytesN<32>) -> Result<Mandate, Error> {
        storage::get_mandate(&env, mandate_id)
    }
}

fn check_mandate(
    env: &Env,
    mandate: &Mandate,
    amount: i128,
    merchant: &Address,
) -> Result<(), Error> {
    if amount <= 0 {
        return Err(Error::InvalidAmount);
    }
    match mandate.status {
        Status::Revoked => return Err(Error::MandateRevoked),
        Status::Exhausted => return Err(Error::BudgetExceeded),
        Status::Active => {}
    }
    if env.ledger().timestamp() >= mandate.expiry {
        return Err(Error::MandateExpired);
    }
    if *merchant != mandate.merchant {
        return Err(Error::MerchantOutOfScope);
    }
    if mandate.spent + amount > mandate.max_amount {
        return Err(Error::BudgetExceeded);
    }
    Ok(())
}

#[cfg(test)]
mod test;

#[cfg(test)]
mod reentry_probe;

#[cfg(test)]
mod hostile_extension;
