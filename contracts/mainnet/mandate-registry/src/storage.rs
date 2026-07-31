//! The only registry module that touches contract-owned storage.

use soroban_sdk::{contracttype, Address, BytesN, Env};

use crate::error::Error;
use crate::mandate::Mandate;

pub const SECONDS_PER_DAY: u64 = 86_400;
pub const MAX_MANDATE_LIFETIME_SECONDS: u64 = 30 * SECONDS_PER_DAY;

// Stellar targets roughly five-second ledgers. A mandate may live for at most
// 30 days; entries and instance state are extended to roughly 60 days.
const DAY_IN_LEDGERS: u32 = 17_280;
const TTL_THRESHOLD: u32 = 7 * DAY_IN_LEDGERS;
const TTL_EXTEND: u32 = 60 * DAY_IN_LEDGERS;

#[contracttype]
pub enum DataKey {
    Paused,
    AllowedAsset(Address),
    Mandate(BytesN<32>),
}

pub fn bump_instance(env: &Env) {
    let extend_to = TTL_EXTEND.min(env.storage().max_ttl());
    let threshold = TTL_THRESHOLD.min(extend_to);
    env.storage().instance().extend_ttl(threshold, extend_to);
}

pub fn set_paused(env: &Env, paused: bool) {
    env.storage().instance().set(&DataKey::Paused, &paused);
}

pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false)
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

pub fn get_mandate(env: &Env, id: BytesN<32>) -> Result<Mandate, Error> {
    let key = DataKey::Mandate(id);
    let mandate = env
        .storage()
        .persistent()
        .get::<DataKey, Mandate>(&key)
        .ok_or(Error::NotFound)?;
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
