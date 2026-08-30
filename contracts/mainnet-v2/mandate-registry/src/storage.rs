//! The ONLY module that touches `env.storage`. Centralizing persistence here
//! means a change to key layout or TTL strategy touches exactly one file.

use soroban_sdk::{contracttype, Address, BytesN, Env};

use crate::error::Error;
use crate::types::Mandate;

pub const SECONDS_PER_DAY: u64 = 86_400;
pub const MAX_MANDATE_LIFETIME_SECONDS: u64 = 30 * SECONDS_PER_DAY;

// Ledger-day constants are approximate because close time is network-configured.
// The accepted mandate lifetime is shorter than the persistence target.
const DAY_IN_LEDGERS: u32 = 17_280;
const TTL_THRESHOLD: u32 = 30 * DAY_IN_LEDGERS;
pub(crate) const TTL_EXTEND: u32 = 120 * DAY_IN_LEDGERS;
pub const SCHEMA_VERSION: u32 = 1;
pub const MANDATE_ID_DOMAIN_VERSION: u32 = 1;

pub fn bump_contract(env: &Env) {
    let extend_to = TTL_EXTEND.min(env.storage().max_ttl());
    let threshold = TTL_THRESHOLD.min(extend_to);
    env.storage().instance().extend_ttl(threshold, extend_to);
}

#[contracttype]
pub enum DataKey {
    SchemaVersion,
    Admin,
    PendingAdmin,
    Paused,
    AllowedAsset(Address),
    UsedCredential(Address, BytesN<32>),
    Mandate(BytesN<32>),
}

pub fn set_schema_version(env: &Env) {
    env.storage()
        .instance()
        .set(&DataKey::SchemaVersion, &SCHEMA_VERSION);
}

pub fn get_schema_version(env: &Env) -> Option<u32> {
    env.storage().instance().get(&DataKey::SchemaVersion)
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

pub fn get_admin(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Admin).unwrap()
}

pub fn set_pending_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::PendingAdmin, admin);
}

pub fn get_pending_admin(env: &Env) -> Option<Address> {
    env.storage().instance().get(&DataKey::PendingAdmin)
}

pub fn clear_pending_admin(env: &Env) {
    env.storage().instance().remove(&DataKey::PendingAdmin);
}

pub fn set_paused(env: &Env, paused: bool) {
    env.storage().instance().set(&DataKey::Paused, &paused);
}

pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::Paused)
        // Missing pause state must fail closed after any future migration.
        .unwrap_or(true)
}

pub fn set_asset_allowed(env: &Env, asset: &Address, allowed: bool) {
    let key = DataKey::AllowedAsset(asset.clone());
    if allowed {
        env.storage().persistent().set(&key, &true);
        bump_persistent(env, &key);
    } else {
        env.storage().persistent().remove(&key);
    }
}

pub fn is_asset_allowed(env: &Env, asset: &Address) -> bool {
    let key = DataKey::AllowedAsset(asset.clone());
    let allowed = env
        .storage()
        .persistent()
        .get::<DataKey, bool>(&key)
        .unwrap_or(false);
    if allowed {
        bump_persistent(env, &key);
    }
    allowed
}

pub fn has_mandate(env: &Env, id: &BytesN<32>) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::Mandate(id.clone()))
}

pub fn has_used_credential(env: &Env, user: &Address, vc_hash: &BytesN<32>) -> bool {
    let key = DataKey::UsedCredential(user.clone(), vc_hash.clone());
    let used = env.storage().persistent().has(&key);
    if used {
        bump_persistent(env, &key);
    }
    used
}

pub fn set_credential_used(env: &Env, user: &Address, vc_hash: &BytesN<32>) {
    let key = DataKey::UsedCredential(user.clone(), vc_hash.clone());
    env.storage().persistent().set(&key, &true);
    bump_persistent(env, &key);
}

pub fn load_mandate(env: &Env, id: BytesN<32>) -> Result<Mandate, Error> {
    let key = DataKey::Mandate(id);
    env.storage()
        .persistent()
        .get::<DataKey, Mandate>(&key)
        .ok_or(Error::NotFound)
}

pub fn get_mandate(env: &Env, id: BytesN<32>) -> Result<Mandate, Error> {
    let key = DataKey::Mandate(id.clone());
    let mandate = load_mandate(env, id)?;
    bump_persistent(env, &key);
    Ok(mandate)
}

pub fn set_mandate(env: &Env, id: &BytesN<32>, mandate: &Mandate) {
    let key = DataKey::Mandate(id.clone());
    env.storage().persistent().set(&key, mandate);
    bump_persistent(env, &key);
}

fn bump_persistent(env: &Env, key: &DataKey) {
    let extend_to = TTL_EXTEND.min(env.storage().max_ttl());
    let threshold = TTL_THRESHOLD.min(extend_to);
    env.storage()
        .persistent()
        .extend_ttl(key, threshold, extend_to);
}
