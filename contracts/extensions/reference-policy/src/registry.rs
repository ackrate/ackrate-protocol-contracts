//! Read-only mirror of Simple MandateRegistry plus its two extension-facing
//! methods. The conformance tests exercise this encoding against the real
//! contract so an interface drift fails before release.

use soroban_sdk::{contractclient, contracttype, Address, BytesN, Env};

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum MandateStatus {
    Active,
    Revoked,
    Exhausted,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct SimpleMandate {
    pub user: Address,
    pub agent: Address,
    pub merchant: Address,
    pub asset: Address,
    pub max_amount: i128,
    pub spent: i128,
    pub expiry: u64,
    pub seq: u32,
    pub status: MandateStatus,
    pub vc_hash: BytesN<32>,
}

#[contractclient(name = "SimpleRegistryClient")]
pub trait SimpleRegistry {
    fn get_mandate(env: Env, mandate_id: BytesN<32>) -> SimpleMandate;
    fn execute_payment(env: Env, mandate_id: BytesN<32>, amount: i128, expected_seq: u32);
}
