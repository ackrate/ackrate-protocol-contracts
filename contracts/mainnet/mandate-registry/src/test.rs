//! Mainnet-candidate integration and continuous negative suite.

#![cfg(test)]

extern crate std;

use reapp_timelock_controller::{TimelockController, TimelockControllerClient};
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::token::{StellarAssetClient, TokenClient};
use soroban_sdk::{vec, Address, BytesN, Env, IntoVal, Symbol, Val, Vec};
use stellar_governance::timelock::OperationState;

use crate::{
    Error, GovernanceConfig, MandateRegistry, MandateRegistryClient, ASSET_POLICY_ROLE,
    MAX_MANDATE_LIFETIME_SECONDS, PAUSER_ROLE, SCHEMA_VERSION, UNPAUSER_ROLE, UPGRADER_ROLE,
};

const NOW: u64 = 1_000;
const EXPIRY: u64 = 10_000;
const MAX: i128 = 50_000_000;
const SPEND: i128 = 10_000_000;
const FUNDED: i128 = 1_000_000_000;

struct World {
    env: Env,
    contract: Address,
    governance: Address,
    pauser: Address,
    unpauser: Address,
    user: Address,
    agent: Address,
    merchant: Address,
    asset: Address,
    id: BytesN<32>,
}

fn setup() -> World {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(NOW);

    let governance = Address::generate(&env);
    let pauser = Address::generate(&env);
    let unpauser = Address::generate(&env);
    let user = Address::generate(&env);
    let agent = Address::generate(&env);
    let merchant = Address::generate(&env);
    let asset = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();

    let config = GovernanceConfig {
        admin: governance.clone(),
        pauser: pauser.clone(),
        unpauser: unpauser.clone(),
        asset_policy: governance.clone(),
        upgrader: governance.clone(),
    };
    let contract = env.register(MandateRegistry, (config, asset.clone()));

    StellarAssetClient::new(&env, &asset).mint(&user, &FUNDED);
    TokenClient::new(&env, &asset).approve(&user, &contract, &FUNDED, &100_000);

    World {
        env: env.clone(),
        contract,
        governance,
        pauser,
        unpauser,
        user,
        agent,
        merchant,
        asset,
        id: BytesN::from_array(&env, &[1; 32]),
    }
}

impl World {
    fn client(&self) -> MandateRegistryClient<'_> {
        MandateRegistryClient::new(&self.env, &self.contract)
    }

    fn register(&self) {
        self.client().register_mandate(
            &self.user,
            &self.agent,
            &self.merchant,
            &self.asset,
            &MAX,
            &EXPIRY,
            &self.id,
        );
    }

    fn balance(&self, who: &Address) -> i128 {
        TokenClient::new(&self.env, &self.asset).balance(who)
    }
}

#[test]
fn constructor_sets_openzeppelin_roles_schema_and_asset() {
    let w = setup();
    let c = w.client();

    assert_eq!(c.get_admin(), Some(w.governance.clone()));
    assert!(c.has_role(&w.pauser, &PAUSER_ROLE).is_some());
    assert!(c.has_role(&w.unpauser, &UNPAUSER_ROLE).is_some());
    assert!(c.has_role(&w.governance, &ASSET_POLICY_ROLE).is_some());
    assert!(c.has_role(&w.governance, &UPGRADER_ROLE).is_some());
    assert_eq!(c.get_schema_version(), SCHEMA_VERSION);
    assert!(c.is_asset_allowed(&w.asset));
    assert!(!c.is_paused());
}

#[test]
fn happy_path_moves_exactly_the_consumed_amount() {
    let w = setup();
    let c = w.client();
    w.register();

    c.validate_mandate(&w.id, &SPEND, &w.merchant);
    c.execute_payment(&w.id, &SPEND, &0);

    let mandate = c.get_mandate(&w.id);
    assert_eq!((mandate.spent, mandate.seq), (SPEND, 1));
    assert_eq!(w.balance(&w.user), FUNDED - SPEND);
    assert_eq!(w.balance(&w.merchant), SPEND);
}

#[test]
fn emergency_pause_is_one_key_but_unpause_is_separate() {
    let w = setup();
    let c = w.client();
    w.register();

    c.pause(&w.pauser);
    assert_eq!(
        c.try_execute_payment(&w.id, &SPEND, &0),
        Err(Ok(Error::Paused))
    );
    assert!(c.try_unpause(&w.pauser).is_err());

    c.unpause(&w.unpauser);
    c.execute_payment(&w.id, &SPEND, &0);
    assert_eq!(w.balance(&w.merchant), SPEND);
}

#[test]
fn governance_functions_require_both_role_and_authorization() {
    let w = setup();
    let c = w.client();
    let attacker = Address::generate(&w.env);
    let other_asset = w
        .env
        .register_stellar_asset_contract_v2(Address::generate(&w.env))
        .address();
    let wasm_hash = BytesN::from_array(&w.env, &[42; 32]);

    assert!(c.try_pause(&attacker).is_err());
    assert!(c
        .try_set_asset_allowed(&other_asset, &true, &attacker)
        .is_err());
    assert!(c.try_upgrade(&wasm_hash, &attacker).is_err());

    w.env.set_auths(&[]);
    assert!(c.try_pause(&w.pauser).is_err());
    assert!(c
        .try_set_asset_allowed(&other_asset, &true, &w.governance)
        .is_err());
    assert!(c.try_upgrade(&wasm_hash, &w.governance).is_err());
}

#[test]
fn asset_allowlist_and_lifetime_are_enforced_at_registration() {
    let w = setup();
    let c = w.client();
    let other_asset = w
        .env
        .register_stellar_asset_contract_v2(Address::generate(&w.env))
        .address();

    assert_eq!(
        c.try_register_mandate(
            &w.user,
            &w.agent,
            &w.merchant,
            &other_asset,
            &MAX,
            &EXPIRY,
            &w.id,
        ),
        Err(Ok(Error::AssetNotAllowed))
    );

    let too_late = NOW + MAX_MANDATE_LIFETIME_SECONDS + 1;
    assert_eq!(
        c.try_register_mandate(
            &w.user,
            &w.agent,
            &w.merchant,
            &w.asset,
            &MAX,
            &too_late,
            &w.id,
        ),
        Err(Ok(Error::MandateTooLong))
    );
}

#[test]
fn canonical_timelock_binds_and_executes_the_exact_policy_change() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(NOW);

    let proposer = Address::generate(&env);
    let pauser = Address::generate(&env);
    let initial_asset = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    let second_asset = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();

    let timelock = env.register(
        TimelockController,
        (
            10u32,
            vec![&env, proposer.clone()],
            Vec::<Address>::new(&env),
            None::<Address>,
        ),
    );
    let registry = env.register(
        MandateRegistry,
        (
            GovernanceConfig {
                admin: timelock.clone(),
                pauser,
                unpauser: proposer.clone(),
                asset_policy: timelock.clone(),
                upgrader: timelock.clone(),
            },
            initial_asset,
        ),
    );

    let timelock_client = TimelockControllerClient::new(&env, &timelock);
    let registry_client = MandateRegistryClient::new(&env, &registry);
    let predecessor = BytesN::from_array(&env, &[0; 32]);
    let salt = BytesN::from_array(&env, &[7; 32]);
    let args: Vec<Val> = vec![
        &env,
        second_asset.clone().into_val(&env),
        true.into_val(&env),
        timelock.clone().into_val(&env),
    ];

    let operation_id = timelock_client.schedule(
        &registry,
        &Symbol::new(&env, "set_asset_allowed"),
        &args,
        &predecessor,
        &salt,
        &10,
        &proposer,
    );
    assert_eq!(
        timelock_client.get_operation_state(&operation_id),
        OperationState::Waiting
    );
    assert!(!registry_client.is_asset_allowed(&second_asset));

    let changed_args: Vec<Val> = vec![
        &env,
        second_asset.clone().into_val(&env),
        false.into_val(&env),
        timelock.clone().into_val(&env),
    ];
    let changed_id = timelock_client.hash_operation(
        &registry,
        &Symbol::new(&env, "set_asset_allowed"),
        &changed_args,
        &predecessor,
        &salt,
    );
    assert_ne!(operation_id, changed_id);

    assert!(timelock_client
        .try_execute(
            &registry,
            &Symbol::new(&env, "set_asset_allowed"),
            &args,
            &predecessor,
            &salt,
            &None,
        )
        .is_err());

    env.ledger().with_mut(|ledger| ledger.sequence_number += 10);
    timelock_client.execute(
        &registry,
        &Symbol::new(&env, "set_asset_allowed"),
        &args,
        &predecessor,
        &salt,
        &None,
    );

    assert!(registry_client.is_asset_allowed(&second_asset));
    assert_eq!(
        timelock_client.get_operation_state(&operation_id),
        OperationState::Done
    );
}

#[test]
fn duplicate_unknown_overspend_expiry_revocation_scope_and_replay_fail() {
    let w = setup();
    let c = w.client();
    w.register();

    assert_eq!(
        c.try_register_mandate(
            &w.user,
            &w.agent,
            &w.merchant,
            &w.asset,
            &MAX,
            &EXPIRY,
            &w.id,
        ),
        Err(Ok(Error::AlreadyExists))
    );

    let unknown = BytesN::from_array(&w.env, &[9; 32]);
    assert_eq!(c.try_get_mandate(&unknown), Err(Ok(Error::NotFound)));
    assert_eq!(
        c.try_execute_payment(&w.id, &(MAX + 1), &0),
        Err(Ok(Error::BudgetExceeded))
    );

    let attacker = Address::generate(&w.env);
    assert_eq!(
        c.try_validate_mandate(&w.id, &SPEND, &attacker),
        Err(Ok(Error::MerchantOutOfScope))
    );

    c.execute_payment(&w.id, &SPEND, &0);
    assert_eq!(
        c.try_execute_payment(&w.id, &SPEND, &0),
        Err(Ok(Error::BadSequence))
    );

    c.revoke_mandate(&w.id);
    assert_eq!(
        c.try_execute_payment(&w.id, &SPEND, &1),
        Err(Ok(Error::MandateRevoked))
    );

    let expired = setup();
    expired.register();
    expired.env.ledger().set_timestamp(EXPIRY);
    assert_eq!(
        expired
            .client()
            .try_execute_payment(&expired.id, &SPEND, &0),
        Err(Ok(Error::MandateExpired))
    );
}

#[test]
fn user_agent_and_revocation_authorizations_are_host_enforced() {
    let w = setup();
    w.register();
    w.env.set_auths(&[]);

    assert!(w.client().try_execute_payment(&w.id, &SPEND, &0).is_err());
    assert!(w.client().try_revoke_mandate(&w.id).is_err());
    assert_eq!(w.balance(&w.merchant), 0);

    let fresh = setup();
    fresh.env.set_auths(&[]);
    assert!(fresh
        .client()
        .try_register_mandate(
            &fresh.user,
            &fresh.agent,
            &fresh.merchant,
            &fresh.asset,
            &MAX,
            &EXPIRY,
            &fresh.id,
        )
        .is_err());
}

#[test]
fn allowance_failure_reverts_consumption_and_transfer() {
    let w = setup();
    TokenClient::new(&w.env, &w.asset).approve(&w.user, &w.contract, &(SPEND - 1), &100_000);
    w.register();

    assert!(w.client().try_execute_payment(&w.id, &SPEND, &0).is_err());
    assert_eq!(w.client().get_mandate(&w.id).spent, 0);
    assert_eq!(w.balance(&w.merchant), 0);
}
