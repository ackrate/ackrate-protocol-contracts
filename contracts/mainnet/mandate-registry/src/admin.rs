//! Governance and emergency controls.
//!
//! The registry contains no scheduler. Contract changes are authorized only by
//! the external canonical TimelockController, which holds the relevant roles.

use soroban_sdk::{symbol_short, Address, BytesN, Env, Symbol};
use stellar_access::access_control::ensure_role;
use stellar_contract_utils::upgradeable;

use crate::{events, storage};

pub const PAUSER_ROLE: Symbol = symbol_short!("pauser");
pub const UNPAUSER_ROLE: Symbol = symbol_short!("unpauser");
pub const ASSET_POLICY_ROLE: Symbol = symbol_short!("assetpol");
pub const UPGRADER_ROLE: Symbol = symbol_short!("upgrader");

fn require_role(env: &Env, role: &Symbol, operator: &Address) {
    operator.require_auth();
    ensure_role(env, role, operator);
}

pub fn pause(env: &Env, operator: Address) {
    storage::bump_instance(env);
    require_role(env, &PAUSER_ROLE, &operator);
    if !storage::is_paused(env) {
        storage::set_paused(env, true);
        events::paused(env, &operator);
    }
}

pub fn unpause(env: &Env, operator: Address) {
    storage::bump_instance(env);
    require_role(env, &UNPAUSER_ROLE, &operator);
    if storage::is_paused(env) {
        storage::set_paused(env, false);
        events::unpaused(env, &operator);
    }
}

pub fn is_paused(env: &Env) -> bool {
    storage::is_paused(env)
}

pub fn set_asset_allowed(env: &Env, asset: Address, allowed: bool, operator: Address) {
    storage::bump_instance(env);
    require_role(env, &ASSET_POLICY_ROLE, &operator);
    storage::set_asset_allowed(env, &asset, allowed);
    events::asset_policy_changed(env, &operator, &asset, allowed);
}

pub fn upgrade(env: &Env, new_wasm_hash: BytesN<32>, operator: Address) {
    storage::bump_instance(env);
    require_role(env, &UPGRADER_ROLE, &operator);
    events::upgraded(env, &operator, &new_wasm_hash);
    upgradeable::upgrade(env, &new_wasm_hash);
}
