//! The only module that touches extension storage.

use soroban_sdk::{contracttype, Address, BytesN, Env};

use crate::types::{InstalledPolicy, MAX_POLICY_LIFETIME_SECS};

const SECS_PER_LEDGER: u64 = 5;
const DAY_IN_LEDGERS: u32 = 17_280;
const TTL_THRESHOLD: u32 = DAY_IN_LEDGERS;
const MIN_TTL_EXTEND: u32 = 30 * DAY_IN_LEDGERS;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Policy(Address, BytesN<32>),
    Consumed(BytesN<32>, BytesN<32>),
    Executing(Address, BytesN<32>),
}

fn policy_key(registry: &Address, mandate_id: &BytesN<32>) -> DataKey {
    DataKey::Policy(registry.clone(), mandate_id.clone())
}

pub fn has_policy(env: &Env, registry: &Address, mandate_id: &BytesN<32>) -> bool {
    env.storage()
        .persistent()
        .has(&policy_key(registry, mandate_id))
}

pub fn set_policy(env: &Env, policy: &InstalledPolicy) {
    let key = policy_key(&policy.binding.registry, &policy.binding.mandate_id);
    env.storage().persistent().set(&key, policy);
    extend_until(env, &key, policy.binding.mandate_expiry);
}

pub fn get_policy(
    env: &Env,
    registry: &Address,
    mandate_id: &BytesN<32>,
) -> Option<InstalledPolicy> {
    let key = policy_key(registry, mandate_id);
    let policy: Option<InstalledPolicy> = env.storage().persistent().get(&key);
    if let Some(ref installed) = policy {
        extend_until(env, &key, installed.binding.mandate_expiry);
    }
    policy
}

fn consumed_key(policy_hash: &BytesN<32>, nonce: &BytesN<32>) -> DataKey {
    DataKey::Consumed(policy_hash.clone(), nonce.clone())
}

pub fn is_consumed(env: &Env, policy_hash: &BytesN<32>, nonce: &BytesN<32>) -> bool {
    env.storage()
        .persistent()
        .has(&consumed_key(policy_hash, nonce))
}

pub fn set_consumed(env: &Env, policy_hash: &BytesN<32>, nonce: &BytesN<32>, mandate_expiry: u64) {
    let key = consumed_key(policy_hash, nonce);
    env.storage().persistent().set(&key, &true);
    extend_until(env, &key, mandate_expiry);
}

fn executing_key(registry: &Address, mandate_id: &BytesN<32>) -> DataKey {
    DataKey::Executing(registry.clone(), mandate_id.clone())
}

pub fn is_executing(env: &Env, registry: &Address, mandate_id: &BytesN<32>) -> bool {
    env.storage()
        .temporary()
        .has(&executing_key(registry, mandate_id))
}

pub fn set_executing(env: &Env, registry: &Address, mandate_id: &BytesN<32>) {
    env.storage()
        .temporary()
        .set(&executing_key(registry, mandate_id), &true);
}

pub fn clear_executing(env: &Env, registry: &Address, mandate_id: &BytesN<32>) {
    env.storage()
        .temporary()
        .remove(&executing_key(registry, mandate_id));
}

fn extend_until(env: &Env, key: &DataKey, expires_at: u64) {
    let remaining = expires_at.saturating_sub(env.ledger().timestamp());
    // Installation rejects any longer policy. Keeping this assertion local to
    // storage prevents a future caller from silently relying on host clamping.
    let bounded = remaining.min(MAX_POLICY_LIFETIME_SECS);
    let doubled = (bounded / SECS_PER_LEDGER).saturating_mul(2);
    let extend = doubled.max(MIN_TTL_EXTEND as u64).min(u32::MAX as u64) as u32;
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, extend);
}
