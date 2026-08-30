#![no_std]
//! Mainnet v2 MandateRegistry.
//!
//! One contract owns the complete enforcement surface: administration,
//! emergency pause, paused-only same-address upgrades, mandate lifecycle, and
//! the single atomic payment path. The SDK and every caller remain untrusted.
//!
//! The implementation is condensed around this file. Persistence and typed
//! errors remain isolated because they define durable compatibility boundaries;
//! data types and events remain small leaf modules.

mod error;
mod events;
mod storage;
mod types;

pub use error::Error;
pub use types::{Mandate, Status};

use soroban_sdk::token::TokenClient;
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env};

#[contract]
pub struct MandateRegistry;

fn require_admin(env: &Env) -> Address {
    let admin = storage::get_admin(env);
    admin.require_auth();
    admin
}

fn checked_spent(
    env: &Env,
    mandate: &Mandate,
    amount: i128,
    merchant: &Address,
) -> Result<i128, Error> {
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

    let new_spent = mandate
        .spent
        .checked_add(amount)
        .ok_or(Error::BudgetExceeded)?;
    if new_spent > mandate.max_amount {
        return Err(Error::BudgetExceeded);
    }
    Ok(new_spent)
}

#[contractimpl]
impl MandateRegistry {
    /// Atomically establishes the initial administrator during deployment.
    /// Constructors run only once; WASM upgrades do not rerun them.
    pub fn __constructor(env: Env, admin: Address) {
        storage::set_schema_version(&env);
        storage::set_admin(&env, &admin);
        storage::set_paused(&env, false);
    }

    /// Current operational administrator.
    pub fn get_admin(env: Env) -> Address {
        storage::get_admin(&env)
    }

    /// Rotate operational authority. Authorized by the current administrator.
    pub fn set_admin(env: Env, new_admin: Address) {
        require_admin(&env);
        storage::set_admin(&env, &new_admin);
        events::admin_set(&env, &new_admin);
    }

    /// Emergency stop for the sole money-moving path.
    pub fn pause(env: Env) {
        let admin = require_admin(&env);
        if !storage::is_paused(&env) {
            storage::set_paused(&env, true);
            events::paused(&env, &admin);
        }
    }

    /// Restore the money-moving path after an emergency stop.
    pub fn unpause(env: Env) {
        let admin = require_admin(&env);
        if storage::is_paused(&env) {
            storage::set_paused(&env, false);
            events::unpaused(&env, &admin);
        }
    }

    /// Read the emergency-stop state without authorization.
    pub fn is_paused(env: Env) -> bool {
        storage::is_paused(&env)
    }

    /// Replace this contract's WASM at the same address. Upgrades require the
    /// administrator's authorization and an already-paused money path.
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) -> Result<(), Error> {
        let admin = require_admin(&env);
        if !storage::is_paused(&env) {
            return Err(Error::UpgradeRequiresPause);
        }

        events::upgraded(&env, &admin, &new_wasm_hash);
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        Ok(())
    }

    /// Store a user-authorized mandate. Mutable fields are initialized by the
    /// contract so the caller cannot seed a spent balance, sequence, or status.
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
        events::mandate_registered(&env, &vc_hash, &user);
        Ok(vc_hash)
    }

    /// Read-only preflight. The authoritative checks are repeated by
    /// `execute_payment` against current stored state.
    pub fn validate_mandate(
        env: Env,
        mandate_id: BytesN<32>,
        amount: i128,
        merchant: Address,
    ) -> Result<(), Error> {
        let mandate = storage::get_mandate(&env, mandate_id)?;
        checked_spent(&env, &mandate, amount, &merchant).map(|_| ())
    }

    /// The only money path. State consumption and transfer are atomic; a token
    /// failure reverts the stored `spent`, `seq`, and status changes.
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
        let new_spent = checked_spent(&env, &mandate, amount, &merchant)?;
        let new_seq = mandate.seq.checked_add(1).ok_or(Error::BadSequence)?;

        mandate.spent = new_spent;
        mandate.seq = new_seq;
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
        events::payment_executed(&env, &mandate_id, &merchant, amount);
        Ok(())
    }

    /// User withdrawal of consent. Authorized by the bound user.
    pub fn revoke_mandate(env: Env, mandate_id: BytesN<32>) -> Result<(), Error> {
        let mut mandate = storage::get_mandate(&env, mandate_id.clone())?;
        mandate.user.require_auth();
        mandate.status = Status::Revoked;
        storage::set_mandate(&env, &mandate_id, &mandate);
        events::mandate_revoked(&env, &mandate_id);
        Ok(())
    }

    /// Read-only accessor for inspection and off-chain preflight.
    pub fn get_mandate(env: Env, mandate_id: BytesN<32>) -> Result<Mandate, Error> {
        storage::get_mandate(&env, mandate_id)
    }
}

#[cfg(test)]
mod test;

#[cfg(test)]
mod reentry_probe;

#[cfg(test)]
mod hostile_extension;
