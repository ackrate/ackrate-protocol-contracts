//! Contract-agent boundary tests.
//!
//! This deliberately untrusted extension applies no extra policy. The tests
//! prove that naming a contract as the mandate agent does not create a second
//! money path or weaken merchant, asset, budget, expiry, revocation, pause,
//! sequence, allowance, or atomic-transfer enforcement.
#![cfg(test)]

use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, Ledger as _},
    token::{StellarAssetClient, TokenClient},
    Address, BytesN, Env,
};

use crate::{Error, GovernanceConfig, MandateRegistry, MandateRegistryClient};

const NOW: u64 = 1_000;
const EXPIRY: u64 = 10_000;
const MAX: i128 = 500;

#[contract]
struct HostileExtension;

#[contractimpl]
impl HostileExtension {
    /// Apply no additional policy and attempt the registry money path directly.
    pub fn execute(
        env: Env,
        registry: Address,
        mandate_id: BytesN<32>,
        amount: i128,
        expected_seq: u32,
    ) {
        MandateRegistryClient::new(&env, &registry).execute_payment(
            &mandate_id,
            &amount,
            &expected_seq,
        );
    }

    /// Attempt to create a second token path. This must fail when the user has
    /// approved only MandateRegistry, which is the required setup.
    pub fn steal(env: Env, asset: Address, user: Address, attacker: Address, amount: i128) {
        TokenClient::new(&env, &asset).transfer_from(
            &env.current_contract_address(),
            &user,
            &attacker,
            &amount,
        );
    }
}

struct World {
    env: Env,
    registry: Address,
    extension: Address,
    user: Address,
    merchant: Address,
    attacker: Address,
    pauser: Address,
    asset: Address,
    mandate_id: BytesN<32>,
}

impl World {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set_timestamp(NOW);
        let extension = env.register(HostileExtension, ());
        let user = Address::generate(&env);
        let merchant = Address::generate(&env);
        let attacker = Address::generate(&env);
        let governance = Address::generate(&env);
        let pauser = Address::generate(&env);
        let unpauser = Address::generate(&env);
        let asset = env
            .register_stellar_asset_contract_v2(Address::generate(&env))
            .address();
        let registry = env.register(
            MandateRegistry,
            (
                GovernanceConfig {
                    admin: governance.clone(),
                    pauser: pauser.clone(),
                    unpauser,
                    asset_policy: governance.clone(),
                    upgrader: governance,
                },
                asset.clone(),
            ),
        );
        let mandate_id = BytesN::from_array(&env, &[91; 32]);

        MandateRegistryClient::new(&env, &registry).register_mandate(
            &user,
            &extension,
            &merchant,
            &asset,
            &MAX,
            &EXPIRY,
            &mandate_id,
        );
        StellarAssetClient::new(&env, &asset).mint(&user, &(MAX * 2));
        TokenClient::new(&env, &asset).approve(&user, &registry, &MAX, &100_000);

        Self {
            env,
            registry,
            extension,
            user,
            merchant,
            attacker,
            pauser,
            asset,
            mandate_id,
        }
    }

    fn registry(&self) -> MandateRegistryClient<'_> {
        MandateRegistryClient::new(&self.env, &self.registry)
    }

    fn hostile(&self) -> HostileExtensionClient<'_> {
        HostileExtensionClient::new(&self.env, &self.extension)
    }

    fn balance(&self, address: &Address) -> i128 {
        TokenClient::new(&self.env, &self.asset).balance(address)
    }
}

#[test]
fn direct_caller_cannot_bypass_a_contract_agent() {
    let world = World::new();
    world.env.set_auths(&[]);

    assert!(world
        .registry()
        .try_execute_payment(&world.mandate_id, &100, &0)
        .is_err());
    assert_eq!(world.balance(&world.merchant), 0);
}

#[test]
fn hostile_extension_is_still_bounded_by_budget_and_sequence() {
    let world = World::new();

    assert!(world
        .hostile()
        .try_execute(&world.registry, &world.mandate_id, &(MAX + 1), &0)
        .is_err());
    assert_eq!(world.registry().get_mandate(&world.mandate_id).spent, 0);

    world
        .hostile()
        .execute(&world.registry, &world.mandate_id, &100, &0);
    assert!(world
        .hostile()
        .try_execute(&world.registry, &world.mandate_id, &100, &0)
        .is_err());
    assert_eq!(world.balance(&world.merchant), 100);
    assert_eq!(world.balance(&world.attacker), 0);
    let mandate = world.registry().get_mandate(&world.mandate_id);
    assert_eq!((mandate.spent, mandate.seq), (100, 1));
}

#[test]
fn hostile_extension_cannot_bypass_revocation_expiry_or_pause() {
    let revoked = World::new();
    revoked.registry().revoke_mandate(&revoked.mandate_id);
    assert!(revoked
        .hostile()
        .try_execute(&revoked.registry, &revoked.mandate_id, &100, &0)
        .is_err());

    let expired = World::new();
    expired.env.ledger().set_timestamp(EXPIRY);
    assert!(expired
        .hostile()
        .try_execute(&expired.registry, &expired.mandate_id, &100, &0)
        .is_err());

    let paused = World::new();
    paused.registry().pause(&paused.pauser);
    assert!(paused
        .hostile()
        .try_execute(&paused.registry, &paused.mandate_id, &100, &0)
        .is_err());

    assert_eq!(revoked.balance(&revoked.merchant), 0);
    assert_eq!(expired.balance(&expired.merchant), 0);
    assert_eq!(paused.balance(&paused.merchant), 0);
}

#[test]
fn hostile_extension_has_no_token_allowance_or_second_money_path() {
    let world = World::new();
    assert_eq!(
        TokenClient::new(&world.env, &world.asset).allowance(&world.user, &world.extension),
        0
    );
    assert!(world
        .hostile()
        .try_steal(&world.asset, &world.user, &world.attacker, &100,)
        .is_err());
    assert_eq!(world.balance(&world.attacker), 0);
    assert_eq!(world.registry().get_mandate(&world.mandate_id).spent, 0);
}

#[test]
fn hostile_extension_cannot_select_a_different_merchant_or_asset() {
    let world = World::new();
    let other_asset = world
        .env
        .register_stellar_asset_contract_v2(Address::generate(&world.env))
        .address();
    StellarAssetClient::new(&world.env, &other_asset).mint(&world.user, &MAX);

    // `execute_payment` exposes no merchant or asset argument. Even the
    // deliberately hostile contract can only request amount and sequence.
    world
        .hostile()
        .execute(&world.registry, &world.mandate_id, &100, &0);

    assert_eq!(world.balance(&world.merchant), 100);
    assert_eq!(world.balance(&world.attacker), 0);
    assert_eq!(
        TokenClient::new(&world.env, &other_asset).balance(&world.attacker),
        0
    );
    assert_eq!(
        world
            .registry()
            .try_validate_mandate(&world.mandate_id, &1, &world.attacker,),
        Err(Ok(Error::MerchantOutOfScope))
    );
}
