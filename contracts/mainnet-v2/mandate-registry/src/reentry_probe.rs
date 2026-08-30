//! Reentrancy regression test — a malicious SEP-41 asset reenters
//! `execute_payment` during `transfer_from`. Soroban's host prohibits contract
//! reentry; this pins that protocol behavior while checks-effects-interactions
//! keeps the registry safe if the external token call fails.
#![cfg(test)]

use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env};

use crate::test::{Principal, PrincipalClient};
use crate::{MandateRegistry, MandateRegistryClient};

// A malicious "token" that, on transfer_from, reenters execute_payment.
#[contract]
pub struct EvilToken;

#[contractimpl]
impl EvilToken {
    pub fn set(env: Env, registry: Address, id: BytesN<32>, amount: i128) {
        env.storage().instance().set(&0u32, &registry);
        env.storage().instance().set(&1u32, &id);
        env.storage().instance().set(&2u32, &amount);
    }

    // SEP-41 surface used by the contract.
    pub fn transfer_from(env: Env, _spender: Address, _from: Address, _to: Address, _amount: i128) {
        let registry: Address = env.storage().instance().get(&0u32).unwrap();
        let id: BytesN<32> = env.storage().instance().get(&1u32).unwrap();
        let amount: i128 = env.storage().instance().get(&2u32).unwrap();
        let c = MandateRegistryClient::new(&env, &registry);
        // Reenter with the *advanced* seq (1) — a "valid" follow-on seq.
        let rejected = c.try_execute_payment(&id, &amount, &1u32).is_err();
        env.storage().instance().set(&3u32, &rejected);
    }
    // Other methods the contract never calls; provide a balance for completeness.
    pub fn balance(_env: Env, _id: Address) -> i128 {
        0
    }

    pub fn reentry_rejected(env: Env) -> bool {
        env.storage().instance().get(&3u32).unwrap_or(false)
    }
}

#[test]
fn reentrancy_via_evil_token() {
    let env = Env::default();
    env.ledger().set_timestamp(1_000);

    let admin = env.register(Principal, ());
    let evil = env.register(EvilToken, ());
    // The custom asset is admitted explicitly so this regression test reaches
    // Soroban's host-level contract-reentry prohibition.
    let registry = env.register(MandateRegistry, (admin, evil.clone()));

    let user = env.register(Principal, ());
    let agent = env.register(Principal, ());
    let merchant = Address::generate(&env);

    let vc_hash = BytesN::from_array(&env, &[7u8; 32]);
    let client = MandateRegistryClient::new(&env, &registry);
    let id = PrincipalClient::new(&env, &user).register(
        &registry,
        &agent,
        &merchant,
        &evil,
        &50_000_000i128,
        &10_000u64,
        &vc_hash,
    );

    // Configure evil token to reenter.
    EvilTokenClient::new(&env, &evil).set(&registry, &id, &10_000_000i128);

    // Outer call: seq 0. Inner reentry tries seq 1.
    PrincipalClient::new(&env, &agent).execute(&registry, &id, &10_000_000i128, &0u32);
    assert!(EvilTokenClient::new(&env, &evil).reentry_rejected());

    let m = client.get_mandate(&id);
    // If host reentry prohibition regressed, spent/seq could advance twice.
    // Panic encodes the observed values into the failure message either way.
    assert_eq!(
        (m.spent, m.seq),
        (10_000_000i128, 1u32),
        "REENTRY OBSERVED: spent/seq differ from single-spend baseline"
    );
}
