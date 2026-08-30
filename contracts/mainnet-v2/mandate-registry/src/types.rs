//! Durable contract data types.

use soroban_sdk::{contracttype, Address, BytesN};

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Mandate {
    /// Principal authorizing the mandate and token allowance.
    pub user: Address,
    /// The only principal permitted to call `execute_payment`.
    pub agent: Address,
    /// The only allowed payment recipient.
    pub merchant: Address,
    /// SEP-41 token contract used for settlement.
    pub asset: Address,
    /// Total amount authorized by the mandate.
    pub max_amount: i128,
    /// Amount consumed so far; always between zero and `max_amount`.
    pub spent: i128,
    /// Ledger-close timestamp after which the mandate is invalid.
    pub expiry: u64,
    /// Monotonic replay guard for successful payments.
    pub seq: u32,
    pub status: Status,
    /// Opaque mandate identifier. User authorization binds this identifier and
    /// every other registration argument to the on-chain invocation.
    pub vc_hash: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Active,
    Revoked,
    Exhausted,
}
