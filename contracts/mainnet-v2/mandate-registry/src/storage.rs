//! The ONLY module that touches `env.storage`. Centralizing persistence here
//! means a change to key layout or TTL strategy touches exactly one file.

use soroban_sdk::{contracttype, Address, BytesN, Env};

use crate::error::Error;
use crate::types::Mandate;

// Ledger-day constants are approximate because close time is network-configured.
// Bump active state when it falls below 30 days and restore it to 120 days.
const DAY_IN_LEDGERS: u32 = 17_280;
const TTL_THRESHOLD: u32 = 30 * DAY_IN_LEDGERS;
pub(crate) const TTL_EXTEND: u32 = 120 * DAY_IN_LEDGERS;
pub const SCHEMA_VERSION: u32 = 1;

fn bump_contract(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(TTL_THRESHOLD, TTL_EXTEND);
}

#[contracttype]
pub enum DataKey {
    SchemaVersion,
    Admin,
    Paused,
    Mandate(BytesN<32>),
}

pub fn set_schema_version(env: &Env) {
    bump_contract(env);
    env.storage()
        .instance()
        .set(&DataKey::SchemaVersion, &SCHEMA_VERSION);
}

#[cfg(test)]
pub fn get_schema_version(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::SchemaVersion)
        .unwrap()
}

pub fn set_admin(env: &Env, admin: &Address) {
    bump_contract(env);
    env.storage().instance().set(&DataKey::Admin, admin);
}

pub fn get_admin(env: &Env) -> Address {
    bump_contract(env);
    env.storage().instance().get(&DataKey::Admin).unwrap()
}

pub fn set_paused(env: &Env, paused: bool) {
    bump_contract(env);
    env.storage().instance().set(&DataKey::Paused, &paused);
}

pub fn is_paused(env: &Env) -> bool {
    bump_contract(env);
    env.storage()
        .instance()
        .get(&DataKey::Paused)
        // Missing pause state must fail closed after any future migration.
        .unwrap_or(true)
}

pub fn has_mandate(env: &Env, id: &BytesN<32>) -> bool {
    bump_contract(env);
    env.storage()
        .persistent()
        .has(&DataKey::Mandate(id.clone()))
}

pub fn get_mandate(env: &Env, id: BytesN<32>) -> Result<Mandate, Error> {
    bump_contract(env);
    let key = DataKey::Mandate(id);
    let mandate = env
        .storage()
        .persistent()
        .get::<DataKey, Mandate>(&key)
        .ok_or(Error::NotFound)?;
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND);
    Ok(mandate)
}

pub fn set_mandate(env: &Env, id: &BytesN<32>, mandate: &Mandate) {
    bump_contract(env);
    let key = DataKey::Mandate(id.clone());
    env.storage().persistent().set(&key, mandate);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND);
}
