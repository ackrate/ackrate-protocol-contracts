use soroban_sdk::{contracttype, Address, BytesN};

pub const INTERFACE_VERSION: u32 = 1;

/// The v1 storage strategy keeps policy and replay entries alive for twice
/// this horizon. The exact mainnet limit remains a release gate: this
/// deliberately conservative bound must be checked against the active network
/// configuration before deployment.
pub const MAX_POLICY_LIFETIME_SECS: u64 = 30 * 86_400;

/// Execution requests are intentionally short-lived even though their nonce is
/// retained for the full policy lifetime.
pub const MAX_REQUEST_LIFETIME_SECS: u64 = 15 * 60;

/// Immutable base-mandate and reference-policy binding.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct PolicyBinding {
    pub version: u32,
    pub network_id: BytesN<32>,
    pub registry: Address,
    pub extension: Address,
    pub mandate_id: BytesN<32>,
    pub user: Address,
    pub merchant: Address,
    pub asset: Address,
    pub mandate_max_amount: i128,
    pub mandate_expiry: u64,
    pub executor: Address,
    pub max_per_payment: i128,
    pub not_before: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct InstalledPolicy {
    pub binding: PolicyBinding,
    pub policy_hash: BytesN<32>,
}

/// Typed, bounded request for the reference policy. More expressive extensions
/// may define a different typed proof, but must preserve these common bindings.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct ExecutionRequest {
    pub version: u32,
    pub network_id: BytesN<32>,
    pub registry: Address,
    pub extension: Address,
    pub mandate_id: BytesN<32>,
    pub amount: i128,
    pub expected_seq: u32,
    pub nonce: BytesN<32>,
    pub valid_after: u64,
    pub valid_before: u64,
    pub policy_hash: BytesN<32>,
}
