//! Typed contract events included in the generated contract specification.

use soroban_sdk::{contractevent, Address, BytesN, Env};

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Paused {
    #[topic]
    pub operator: Address,
}

pub fn paused(env: &Env, operator: &Address) {
    Paused {
        operator: operator.clone(),
    }
    .publish(env);
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unpaused {
    #[topic]
    pub operator: Address,
}

pub fn unpaused(env: &Env, operator: &Address) {
    Unpaused {
        operator: operator.clone(),
    }
    .publish(env);
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetPolicyChanged {
    #[topic]
    pub asset: Address,
    pub allowed: bool,
    pub operator: Address,
}

pub fn asset_policy_changed(env: &Env, operator: &Address, asset: &Address, allowed: bool) {
    AssetPolicyChanged {
        asset: asset.clone(),
        allowed,
        operator: operator.clone(),
    }
    .publish(env);
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Upgraded {
    #[topic]
    pub operator: Address,
    pub wasm_hash: BytesN<32>,
}

pub fn upgraded(env: &Env, operator: &Address, wasm_hash: &BytesN<32>) {
    Upgraded {
        operator: operator.clone(),
        wasm_hash: wasm_hash.clone(),
    }
    .publish(env);
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MandateRegistered {
    #[topic]
    pub user: Address,
    pub mandate_id: BytesN<32>,
}

pub fn mandate_registered(env: &Env, mandate_id: &BytesN<32>, user: &Address) {
    MandateRegistered {
        user: user.clone(),
        mandate_id: mandate_id.clone(),
    }
    .publish(env);
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentExecuted {
    #[topic]
    pub merchant: Address,
    pub mandate_id: BytesN<32>,
    pub amount: i128,
}

pub fn payment_executed(env: &Env, mandate_id: &BytesN<32>, merchant: &Address, amount: i128) {
    PaymentExecuted {
        merchant: merchant.clone(),
        mandate_id: mandate_id.clone(),
        amount,
    }
    .publish(env);
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MandateRevoked {
    pub mandate_id: BytesN<32>,
}

pub fn mandate_revoked(env: &Env, mandate_id: &BytesN<32>) {
    MandateRevoked {
        mandate_id: mandate_id.clone(),
    }
    .publish(env);
}
