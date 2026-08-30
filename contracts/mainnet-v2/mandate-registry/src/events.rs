//! Typed events with schemas embedded in the generated contract specification.

use soroban_sdk::{contractevent, Address, BytesN, Env};

#[contractevent(topics = ["admin"], data_format = "single-value")]
pub struct AdminSet {
    pub new_admin: Address,
}

#[contractevent(topics = ["admin_pending"], data_format = "single-value")]
pub struct AdminTransferProposed {
    pub pending_admin: Address,
}

#[contractevent(topics = ["asset_policy"], data_format = "single-value")]
pub struct AssetPolicyChanged {
    #[topic]
    pub asset: Address,
    pub allowed: bool,
}

#[contractevent(topics = ["paused"], data_format = "single-value")]
pub struct Paused {
    #[topic]
    pub admin: Address,
    pub data: (),
}

#[contractevent(topics = ["unpaused"], data_format = "single-value")]
pub struct Unpaused {
    #[topic]
    pub admin: Address,
    pub data: (),
}

#[contractevent(topics = ["upgrade"], data_format = "single-value")]
pub struct Upgraded {
    #[topic]
    pub admin: Address,
    pub wasm_hash: BytesN<32>,
}

#[contractevent(topics = ["register"], data_format = "single-value")]
pub struct MandateRegistered {
    #[topic]
    pub user: Address,
    pub mandate_id: BytesN<32>,
}

#[contractevent(topics = ["payment"], data_format = "vec")]
pub struct PaymentExecuted {
    #[topic]
    pub merchant: Address,
    #[topic]
    pub asset: Address,
    pub mandate_id: BytesN<32>,
    pub amount: i128,
    pub sequence: u32,
}

#[contractevent(topics = ["revoke"], data_format = "single-value")]
pub struct MandateRevoked {
    pub mandate_id: BytesN<32>,
}

pub fn admin_set(env: &Env, new_admin: &Address) {
    AdminSet {
        new_admin: new_admin.clone(),
    }
    .publish(env);
}

pub fn admin_transfer_proposed(env: &Env, pending_admin: &Address) {
    AdminTransferProposed {
        pending_admin: pending_admin.clone(),
    }
    .publish(env);
}

pub fn asset_policy_changed(env: &Env, asset: &Address, allowed: bool) {
    AssetPolicyChanged {
        asset: asset.clone(),
        allowed,
    }
    .publish(env);
}

pub fn paused(env: &Env, admin: &Address) {
    Paused {
        admin: admin.clone(),
        data: (),
    }
    .publish(env);
}

pub fn unpaused(env: &Env, admin: &Address) {
    Unpaused {
        admin: admin.clone(),
        data: (),
    }
    .publish(env);
}

pub fn upgraded(env: &Env, admin: &Address, wasm_hash: &BytesN<32>) {
    Upgraded {
        admin: admin.clone(),
        wasm_hash: wasm_hash.clone(),
    }
    .publish(env);
}

pub fn mandate_registered(env: &Env, mandate_id: &BytesN<32>, user: &Address) {
    MandateRegistered {
        user: user.clone(),
        mandate_id: mandate_id.clone(),
    }
    .publish(env);
}

pub fn payment_executed(
    env: &Env,
    mandate_id: &BytesN<32>,
    merchant: &Address,
    asset: &Address,
    amount: i128,
    sequence: u32,
) {
    PaymentExecuted {
        merchant: merchant.clone(),
        asset: asset.clone(),
        mandate_id: mandate_id.clone(),
        amount,
        sequence,
    }
    .publish(env);
}

pub fn mandate_revoked(env: &Env, mandate_id: &BytesN<32>) {
    MandateRevoked {
        mandate_id: mandate_id.clone(),
    }
    .publish(env);
}
