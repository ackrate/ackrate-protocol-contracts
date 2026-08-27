#![no_std]
//! MandateRegistry — ACKRATE's on-chain enforcement layer.
//!
//! The contract is the entire protocol and is small by design: a small
//! interface is reviewable. Money moves only through `execute_payment`, which
//! validates-and-consumes the mandate atomically before transferring. The SDK
//! is untrusted; this contract is the source of truth.
//!
//! Module responsibilities (dependencies flow ONE way, no cycles):
//!
//!   lib  →  {registry, payment}  →  storage  →  mandate / error
//!                  └────────────→  events  (leaf; anyone may emit)
//!
//!  - `lib`      — contract entry points only: thin dispatch, no logic.
//!  - `mandate`  — the `Mandate` type (pure data).
//!  - `storage`  — keys + all get/set/TTL (the ONLY module touching registry storage).
//!  - `registry` — register / revoke (allowance funding model).
//!  - `payment`  — validate_mandate + execute_payment + the token transfer.
//!  - `error`    — typed errors.
//!  - `events`   — emitted events.

mod admin;
mod error;
mod events;
mod mandate;
mod payment;
mod registry;
mod storage;

pub use error::Error;
pub use mandate::{Mandate, Status};

use soroban_sdk::{contract, contractimpl, contracttype, Address, BytesN, Env, Symbol, Vec};
use stellar_access::access_control::{grant_role_no_auth, set_admin, AccessControl};
use stellar_contract_utils::upgradeable::{self, Upgradeable};

pub use admin::{ASSET_POLICY_ROLE, PAUSER_ROLE, UNPAUSER_ROLE, UPGRADER_ROLE};
pub use storage::MAX_MANDATE_LIFETIME_SECONDS;

pub const SCHEMA_VERSION: u32 = 1;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GovernanceConfig {
    /// OpenZeppelin top administrator. In the deployment profile this is the
    /// canonical timelock controller.
    pub admin: Address,
    /// Emergency key allowed only to stop the money path.
    pub pauser: Address,
    /// Selected 2-of-3 authority allowed to restore the money path.
    pub unpauser: Address,
    /// Timelock controller allowed to modify the asset allowlist.
    pub asset_policy: Address,
    /// Timelock controller allowed to replace this contract's WASM.
    pub upgrader: Address,
}

#[contract]
pub struct MandateRegistry;

#[contractimpl]
impl MandateRegistry {
    /// Establish immutable-at-construction governance roots and the first
    /// allowed asset. Constructors run only once and never run on upgrade.
    pub fn __constructor(env: &Env, governance: GovernanceConfig, initial_asset: Address) {
        set_admin(env, &governance.admin);
        grant_role_no_auth(env, &governance.pauser, &PAUSER_ROLE, &governance.admin);
        grant_role_no_auth(env, &governance.unpauser, &UNPAUSER_ROLE, &governance.admin);
        grant_role_no_auth(
            env,
            &governance.asset_policy,
            &ASSET_POLICY_ROLE,
            &governance.admin,
        );
        grant_role_no_auth(env, &governance.upgrader, &UPGRADER_ROLE, &governance.admin);
        storage::set_paused(env, false);
        storage::set_asset_allowed(env, &initial_asset, true);
        upgradeable::set_schema_version(env, SCHEMA_VERSION);
        storage::bump_instance(env);
    }

    /// Emergency stop for the sole money-moving path.
    pub fn pause(env: Env, operator: Address) {
        admin::pause(&env, operator)
    }

    /// Restore the money-moving path. The deployment profile assigns this role
    /// to the selected 2-of-3 authority, never to the one-key pauser.
    pub fn unpause(env: Env, operator: Address) {
        admin::unpause(&env, operator)
    }

    /// Read the emergency-stop state without authorization.
    pub fn is_paused(env: Env) -> bool {
        storage::bump_instance(&env);
        admin::is_paused(&env)
    }

    /// Change the admission policy for new mandates. Existing user-signed
    /// mandates are not rewritten. The deployment profile assigns this role
    /// only to the canonical timelock; `pause` is the emergency execution stop.
    pub fn set_asset_allowed(env: Env, asset: Address, allowed: bool, operator: Address) {
        admin::set_asset_allowed(&env, asset, allowed, operator)
    }

    pub fn is_asset_allowed(env: Env, asset: Address) -> bool {
        storage::bump_instance(&env);
        storage::is_asset_allowed(&env, &asset)
    }

    pub fn get_schema_version(env: Env) -> u32 {
        storage::bump_instance(&env);
        upgradeable::get_schema_version(&env)
    }

    /// Permissionless maintenance for the contract instance and code TTL.
    pub fn keep_alive(env: Env) {
        storage::bump_instance(&env);
    }

    pub fn role_ids(_env: Env) -> (Symbol, Symbol, Symbol, Symbol) {
        (PAUSER_ROLE, UNPAUSER_ROLE, ASSET_POLICY_ROLE, UPGRADER_ROLE)
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
        storage::bump_instance(&env);
        registry::register_mandate(
            &env, user, agent, merchant, asset, max_amount, expiry, vc_hash,
        )
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
        storage::bump_instance(&env);
        payment::validate_mandate(&env, mandate_id, amount, merchant)
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
        storage::bump_instance(&env);
        payment::execute_payment(&env, mandate_id, amount, expected_seq)
    }

    /// User withdraws consent; marks the mandate Revoked. Authorized by the user.
    pub fn revoke_mandate(env: Env, mandate_id: BytesN<32>) -> Result<(), Error> {
        storage::bump_instance(&env);
        registry::revoke_mandate(&env, mandate_id)
    }

    /// Read-only accessor for the stored mandate (inspection / preflight).
    pub fn get_mandate(env: Env, mandate_id: BytesN<32>) -> Result<Mandate, Error> {
        storage::bump_instance(&env);
        storage::get_mandate(&env, mandate_id)
    }
}

#[contractimpl]
impl Upgradeable for MandateRegistry {
    fn upgrade(env: &Env, new_wasm_hash: BytesN<32>, operator: Address) {
        admin::upgrade(env, new_wasm_hash, operator);
    }
}

#[contractimpl(contracttrait)]
impl AccessControl for MandateRegistry {}

#[cfg(test)]
mod test;

#[cfg(test)]
mod reentry_probe;

#[cfg(test)]
mod hostile_extension;
