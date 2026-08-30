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
pub use storage::MAX_MANDATE_LIFETIME_SECONDS;
pub use types::{Mandate, Status};

use soroban_sdk::token::TokenClient;
use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env};

use types::MandateIdPreimage;

#[contract]
pub struct MandateRegistry;

fn require_admin(env: &Env) -> Address {
    let admin = storage::get_admin(env);
    admin.require_auth();
    admin
}

fn require_current_schema(env: &Env) -> Result<(), Error> {
    if storage::get_schema_version(env) == Some(storage::SCHEMA_VERSION) {
        Ok(())
    } else {
        Err(Error::InvalidState)
    }
}

#[allow(clippy::too_many_arguments)]
fn derive_id(
    env: &Env,
    user: &Address,
    agent: &Address,
    merchant: &Address,
    asset: &Address,
    max_amount: i128,
    expiry: u64,
    vc_hash: &BytesN<32>,
) -> BytesN<32> {
    let preimage = MandateIdPreimage {
        version: storage::MANDATE_ID_DOMAIN_VERSION,
        network_id: env.ledger().network_id(),
        registry: env.current_contract_address(),
        user: user.clone(),
        agent: agent.clone(),
        merchant: merchant.clone(),
        asset: asset.clone(),
        max_amount,
        expiry,
        vc_hash: vc_hash.clone(),
    };
    env.crypto().sha256(&preimage.to_xdr(env)).into()
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
    if mandate.max_amount <= 0
        || mandate.spent < 0
        || mandate.spent > mandate.max_amount
        || (mandate.status == Status::Active && mandate.spent == mandate.max_amount)
        || (mandate.status == Status::Exhausted && mandate.spent != mandate.max_amount)
    {
        return Err(Error::InvalidState);
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
    if !storage::is_asset_allowed(env, &mandate.asset) {
        return Err(Error::AssetNotAllowed);
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
    pub fn __constructor(env: Env, admin: Address, initial_asset: Address) {
        storage::bump_contract(&env);
        storage::set_schema_version(&env);
        storage::set_admin(&env, &admin);
        storage::set_paused(&env, false);
        storage::set_asset_allowed(&env, &initial_asset, true);
    }

    /// Current operational administrator.
    pub fn get_admin(env: Env) -> Address {
        storage::bump_contract(&env);
        storage::get_admin(&env)
    }

    /// Candidate administrator waiting to accept a proposed handoff.
    pub fn get_pending_admin(env: Env) -> Option<Address> {
        storage::bump_contract(&env);
        storage::get_pending_admin(&env)
    }

    /// Propose a recoverable authority handoff. The current administrator keeps
    /// control until the candidate proves control by calling `accept_admin`.
    pub fn propose_admin(env: Env, new_admin: Address) {
        storage::bump_contract(&env);
        require_admin(&env);
        storage::set_pending_admin(&env, &new_admin);
        events::admin_transfer_proposed(&env, &new_admin);
    }

    /// Accept a pending handoff. Authorized by the proposed administrator.
    pub fn accept_admin(env: Env) -> Result<(), Error> {
        storage::bump_contract(&env);
        let pending = storage::get_pending_admin(&env).ok_or(Error::NoPendingAdmin)?;
        pending.require_auth();
        storage::set_admin(&env, &pending);
        storage::clear_pending_admin(&env);
        events::admin_set(&env, &pending);
        Ok(())
    }

    /// Emergency stop for the sole money-moving path.
    pub fn pause(env: Env) {
        storage::bump_contract(&env);
        let admin = require_admin(&env);
        if !storage::is_paused(&env) {
            storage::set_paused(&env, true);
            events::paused(&env, &admin);
        }
    }

    /// Restore the money-moving path after an emergency stop.
    pub fn unpause(env: Env) {
        storage::bump_contract(&env);
        let admin = require_admin(&env);
        if storage::is_paused(&env) {
            storage::set_paused(&env, false);
            events::unpaused(&env, &admin);
        }
    }

    /// Read the emergency-stop state without authorization.
    pub fn is_paused(env: Env) -> bool {
        storage::bump_contract(&env);
        storage::is_paused(&env)
    }

    /// Change the reviewed-token admission policy. Policy changes are allowed
    /// only while the money path is paused; removal also blocks existing
    /// mandates from executing against that asset.
    pub fn set_asset_allowed(env: Env, asset: Address, allowed: bool) -> Result<(), Error> {
        storage::bump_contract(&env);
        require_admin(&env);
        if !storage::is_paused(&env) {
            return Err(Error::AssetPolicyRequiresPause);
        }
        storage::set_asset_allowed(&env, &asset, allowed);
        events::asset_policy_changed(&env, &asset, allowed);
        Ok(())
    }

    /// Whether an asset is currently admitted for validation and settlement.
    pub fn is_asset_allowed(env: Env, asset: Address) -> bool {
        storage::bump_contract(&env);
        storage::is_asset_allowed(&env, &asset)
    }

    /// Current durable storage schema. Money-path methods reject a missing or
    /// unexpected version so an incompatible upgrade cannot fail open.
    pub fn get_schema_version(env: Env) -> Result<u32, Error> {
        storage::bump_contract(&env);
        storage::get_schema_version(&env).ok_or(Error::InvalidState)
    }

    /// Replace this contract's WASM at the same address. Upgrades require the
    /// administrator's authorization and an already-paused money path.
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) -> Result<(), Error> {
        storage::bump_contract(&env);
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
        storage::bump_contract(&env);
        require_current_schema(&env)?;
        user.require_auth();
        if max_amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        if expiry <= env.ledger().timestamp() {
            return Err(Error::MandateExpired);
        }
        if expiry - env.ledger().timestamp() > storage::MAX_MANDATE_LIFETIME_SECONDS {
            return Err(Error::MandateTooLong);
        }
        if !storage::is_asset_allowed(&env, &asset) {
            return Err(Error::AssetNotAllowed);
        }
        if storage::has_used_credential(&env, &user, &vc_hash) {
            return Err(Error::AlreadyExists);
        }
        let mandate_id = derive_id(
            &env, &user, &agent, &merchant, &asset, max_amount, expiry, &vc_hash,
        );
        if storage::has_mandate(&env, &mandate_id) {
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
        storage::set_mandate(&env, &mandate_id, &mandate);
        storage::set_credential_used(&env, &user, &vc_hash);
        events::mandate_registered(&env, &mandate_id, &user);
        Ok(mandate_id)
    }

    /// Deterministically derive the domain-separated identifier returned by
    /// `register_mandate` without depending on any x402 wire representation.
    #[allow(clippy::too_many_arguments)]
    pub fn derive_mandate_id(
        env: Env,
        user: Address,
        agent: Address,
        merchant: Address,
        asset: Address,
        max_amount: i128,
        expiry: u64,
        vc_hash: BytesN<32>,
    ) -> BytesN<32> {
        storage::bump_contract(&env);
        derive_id(
            &env, &user, &agent, &merchant, &asset, max_amount, expiry, &vc_hash,
        )
    }

    /// Non-value-moving preview. It may refresh TTLs; the authoritative checks
    /// are repeated by `execute_payment` against current stored state.
    pub fn validate_mandate(
        env: Env,
        mandate_id: BytesN<32>,
        amount: i128,
        expected_seq: u32,
        merchant: Address,
        asset: Address,
    ) -> Result<(), Error> {
        storage::bump_contract(&env);
        require_current_schema(&env)?;
        if storage::is_paused(&env) {
            return Err(Error::Paused);
        }
        let mandate = storage::get_mandate(&env, mandate_id)?;
        if expected_seq != mandate.seq {
            return Err(Error::BadSequence);
        }
        mandate.seq.checked_add(1).ok_or(Error::SequenceExhausted)?;
        if asset != mandate.asset {
            return Err(Error::AssetOutOfScope);
        }
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
        storage::bump_contract(&env);
        require_current_schema(&env)?;
        if storage::is_paused(&env) {
            return Err(Error::Paused);
        }

        let mut mandate = storage::load_mandate(&env, mandate_id.clone())?;
        mandate.agent.require_auth();
        if expected_seq != mandate.seq {
            return Err(Error::BadSequence);
        }

        let merchant = mandate.merchant.clone();
        let new_spent = checked_spent(&env, &mandate, amount, &merchant)?;
        let consumed_sequence = mandate.seq;
        let new_seq = mandate.seq.checked_add(1).ok_or(Error::SequenceExhausted)?;

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
        events::payment_executed(
            &env,
            &mandate_id,
            &merchant,
            &mandate.asset,
            amount,
            consumed_sequence,
        );
        Ok(())
    }

    /// User withdrawal of consent. Authorized by the bound user.
    pub fn revoke_mandate(env: Env, mandate_id: BytesN<32>) -> Result<(), Error> {
        storage::bump_contract(&env);
        require_current_schema(&env)?;
        let mut mandate = storage::load_mandate(&env, mandate_id.clone())?;
        mandate.user.require_auth();
        mandate.status = Status::Revoked;
        storage::set_mandate(&env, &mandate_id, &mandate);
        events::mandate_revoked(&env, &mandate_id);
        Ok(())
    }

    /// Non-value-moving accessor for inspection and off-chain preflight. A
    /// successful read may refresh the mandate's persistence horizon.
    pub fn get_mandate(env: Env, mandate_id: BytesN<32>) -> Result<Mandate, Error> {
        storage::bump_contract(&env);
        require_current_schema(&env)?;
        storage::get_mandate(&env, mandate_id)
    }
}

#[cfg(test)]
mod test;

#[cfg(test)]
mod reentry_probe;

#[cfg(test)]
mod hostile_extension;
