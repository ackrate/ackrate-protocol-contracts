//! Durable contract data types.

use soroban_sdk::{contracttype, Address, BytesN};

/// Canonical, wire-format-neutral preimage for a mandate's on-chain identifier.
/// Network and registry domains prevent an identifier from being replayed across
/// deployments; the immutable terms prevent another user from squatting a
/// disclosed credential hash under different payment terms.
#[contracttype]
#[derive(Clone)]
pub(crate) struct MandateIdPreimage {
    pub version: u32,
    pub network_id: BytesN<32>,
    pub registry: Address,
    pub user: Address,
    pub agent: Address,
    pub merchant: Address,
    pub asset: Address,
    pub max_amount: i128,
    pub expiry: u64,
    pub vc_hash: BytesN<32>,
}

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
    /// Credential commitment and caller-supplied uniqueness source. The
    /// on-chain mandate identifier is a domain-separated hash over this value
    /// and every immutable mandate term.
    pub vc_hash: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Active,
    Revoked,
    Exhausted,
}
