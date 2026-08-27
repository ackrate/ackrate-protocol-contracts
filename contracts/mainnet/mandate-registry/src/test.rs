//! Mainnet-candidate integration and continuous negative suite.

#![cfg(test)]

extern crate std;

use ackrate_timelock_controller::{TimelockController, TimelockControllerClient};
use soroban_sdk::testutils::{Address as _, Deployer as _, Ledger as _};
use soroban_sdk::token::{StellarAssetClient, TokenClient};
use soroban_sdk::{symbol_short, vec, Address, Bytes, BytesN, Env, IntoVal, Symbol, Val, Vec};
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

// Protocol-21 `add(u64,u64)->u64` fixture from soroban-sdk 22.0.11. Keeping
// the small replacement as text avoids a generated binary or build script.
const REPLACEMENT_WASM_HEX: &str = "0061736d0100000001140460017e017e60027f7e0060027e7e017e600000020d020169013000000169015f0000030605010203030305030100100619037f01418080c0000b7f00418080c0000b7f00418080c0000b072f05066d656d6f72790200036164640003015f00060a5f5f646174615f656e6403010b5f5f686561705f6261736503020a8c02055d02017f017e024002402001a741ff0171220241c000460d00024020024106460d00420121034283908080800121010c020b20014208882101420021030c010b42002103200110808080800021010b20002001370308200020033703000b990101017f23808080800041206b2202248080808000200241106a20001082808080000240024020022802100d0020022903182100200220011082808080002002290300a70d00200020022903087c22012000540d0102400240200142ffffffffffffffff00560d00200142088642068421000c010b200110818080800021000b200241206a24808080800020000f0b00000b108480808000000b0900108580808000000b040000000b02000b004b0e636f6e7472616374737065637630000000000000000000000003616464000000000200000000000000016100000000000006000000000000000162000000000000060000000100000006001e11636f6e7472616374656e766d6574617630000000000000001500000000007b0e636f6e74726163746d65746176300000000000000005727376657200000000000006312e37342e3000000000000000000008727373646b7665720000003932312e302e312d707265766965772e312331313663333562633965303366346231623565363562356565383331616530663836616139326664000000";

fn replacement_wasm(env: &Env) -> Bytes {
    fn nibble(value: u8) -> u8 {
        match value {
            b'0'..=b'9' => value - b'0',
            b'a'..=b'f' => value - b'a' + 10,
            _ => panic!("invalid replacement WASM hex"),
        }
    }
    let encoded = REPLACEMENT_WASM_HEX.as_bytes();
    let mut wasm = Bytes::new(env);
    let mut index = 0;
    while index < encoded.len() {
        wasm.push_back((nibble(encoded[index]) << 4) | nibble(encoded[index + 1]));
        index += 2;
    }
    wasm
}

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
fn inherited_access_control_mutators_reject_wrong_authority_and_missing_auth() {
    let w = setup();
    let c = w.client();
    let attacker = Address::generate(&w.env);
    let candidate = Address::generate(&w.env);

    // Explicit-caller mutators reject an address that is neither the contract
    // admin nor the configured admin role for the target role.
    assert!(c
        .try_grant_role(&candidate, &PAUSER_ROLE, &attacker)
        .is_err());
    assert!(c
        .try_revoke_role(&w.pauser, &PAUSER_ROLE, &attacker)
        .is_err());
    assert!(c.try_renounce_role(&PAUSER_ROLE, &attacker).is_err());

    // Establish a real pending transfer while authorization is available so
    // accept_admin_transfer reaches its recipient-authentication boundary.
    c.transfer_admin_role(&candidate, &100);

    // Every inherited state-changing entry point must still fail when its
    // otherwise-correct authority omits the host-enforced signature.
    w.env.set_auths(&[]);
    assert!(c
        .try_grant_role(&candidate, &PAUSER_ROLE, &w.governance)
        .is_err());
    assert!(c
        .try_revoke_role(&w.pauser, &PAUSER_ROLE, &w.governance)
        .is_err());
    assert!(c.try_renounce_role(&PAUSER_ROLE, &w.pauser).is_err());
    assert!(c.try_set_role_admin(&PAUSER_ROLE, &UNPAUSER_ROLE).is_err());
    assert!(c.try_transfer_admin_role(&candidate, &100).is_err());
    assert!(c.try_accept_admin_transfer().is_err());
    assert!(c.try_renounce_admin().is_err());
}

#[test]
fn inherited_access_control_grant_revoke_and_read_surface_are_consistent() {
    let w = setup();
    let c = w.client();
    let candidate = Address::generate(&w.env);

    assert_eq!(
        c.role_ids(),
        (PAUSER_ROLE, UNPAUSER_ROLE, ASSET_POLICY_ROLE, UPGRADER_ROLE)
    );
    assert_eq!(c.get_role_member_count(&PAUSER_ROLE), 1);
    assert_eq!(c.get_role_member(&PAUSER_ROLE, &0), w.pauser);
    assert_eq!(c.get_role_admin(&PAUSER_ROLE), None);
    assert_eq!(c.get_role_member_count(&symbol_short!("unknown")), 0);
    assert_eq!(c.get_role_admin(&symbol_short!("unknown")), None);
    assert!(c.try_get_role_member(&PAUSER_ROLE, &1).is_err());

    let roles = c.get_existing_roles();
    for role in [PAUSER_ROLE, UNPAUSER_ROLE, ASSET_POLICY_ROLE, UPGRADER_ROLE] {
        assert!(roles.iter().any(|existing| existing == role));
    }

    c.grant_role(&candidate, &PAUSER_ROLE, &w.governance);
    assert_eq!(c.has_role(&candidate, &PAUSER_ROLE), Some(1));
    assert_eq!(c.get_role_member_count(&PAUSER_ROLE), 2);
    assert_eq!(c.get_role_member(&PAUSER_ROLE, &1), candidate);

    c.revoke_role(&w.pauser, &PAUSER_ROLE, &w.governance);
    assert_eq!(c.has_role(&w.pauser, &PAUSER_ROLE), None);
    assert_eq!(c.get_role_member_count(&PAUSER_ROLE), 1);
    assert_eq!(c.get_role_member(&PAUSER_ROLE, &0), candidate);
    assert!(c.try_pause(&w.pauser).is_err());
}

#[test]
fn inherited_access_control_role_admin_delegation_and_renunciation_work() {
    let w = setup();
    let c = w.client();
    let delegate = Address::generate(&w.env);
    let candidate = Address::generate(&w.env);
    let attacker = Address::generate(&w.env);
    let delegated_admin_role = symbol_short!("pauseadm");

    c.grant_role(&delegate, &delegated_admin_role, &w.governance);
    c.set_role_admin(&PAUSER_ROLE, &delegated_admin_role);
    assert_eq!(c.get_role_admin(&PAUSER_ROLE), Some(delegated_admin_role));

    c.grant_role(&candidate, &PAUSER_ROLE, &delegate);
    assert!(c.has_role(&candidate, &PAUSER_ROLE).is_some());
    assert!(c
        .try_revoke_role(&candidate, &PAUSER_ROLE, &attacker)
        .is_err());
    c.revoke_role(&candidate, &PAUSER_ROLE, &delegate);
    assert_eq!(c.has_role(&candidate, &PAUSER_ROLE), None);

    c.renounce_role(&PAUSER_ROLE, &w.pauser);
    assert_eq!(c.has_role(&w.pauser, &PAUSER_ROLE), None);
    assert!(c.try_pause(&w.pauser).is_err());
}

#[test]
fn inherited_access_control_admin_transfer_and_renunciation_are_enforced() {
    let w = setup();
    let c = w.client();
    let candidate = Address::generate(&w.env);
    let role_holder = Address::generate(&w.env);

    c.transfer_admin_role(&candidate, &100);
    assert_eq!(c.get_admin(), Some(w.governance.clone()));
    assert!(c.try_renounce_admin().is_err());
    c.accept_admin_transfer();
    assert_eq!(c.get_admin(), Some(candidate.clone()));
    assert!(c
        .try_grant_role(&role_holder, &PAUSER_ROLE, &w.governance)
        .is_err());
    c.grant_role(&role_holder, &PAUSER_ROLE, &candidate);
    assert!(c.has_role(&role_holder, &PAUSER_ROLE).is_some());

    c.renounce_admin();
    assert_eq!(c.get_admin(), None);
    assert!(c
        .try_grant_role(&Address::generate(&w.env), &PAUSER_ROLE, &candidate)
        .is_err());
}

#[test]
fn expired_admin_transfer_cannot_be_accepted() {
    let w = setup();
    let c = w.client();
    let candidate = Address::generate(&w.env);

    c.transfer_admin_role(&candidate, &1);
    w.env.ledger().with_mut(|ledger| ledger.sequence_number = 2);
    assert!(c.try_accept_admin_transfer().is_err());
    assert_eq!(c.get_admin(), Some(w.governance));
}

#[test]
fn keep_alive_extends_contract_instance_ttl() {
    let w = setup();
    let c = w.client();
    let initial_ttl = w.env.deployer().get_contract_instance_ttl(&w.contract);
    assert!(initial_ttl > 10);

    w.env
        .ledger()
        .with_mut(|ledger| ledger.sequence_number += initial_ttl - 10);
    let ttl_before = w.env.deployer().get_contract_instance_ttl(&w.contract);
    c.keep_alive();
    let ttl_after = w.env.deployer().get_contract_instance_ttl(&w.contract);

    assert!(ttl_before <= 10);
    assert!(ttl_after > ttl_before);
}

#[test]
fn unpause_requires_the_unpauser_signature() {
    let w = setup();
    let c = w.client();
    c.pause(&w.pauser);
    w.env.set_auths(&[]);

    assert!(c.try_unpause(&w.unpauser).is_err());
    assert!(c.is_paused());
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
        c.try_register_mandate(&w.user, &w.agent, &w.merchant, &w.asset, &0, &EXPIRY, &w.id,),
        Err(Ok(Error::InvalidAmount))
    );
    assert_eq!(
        c.try_register_mandate(&w.user, &w.agent, &w.merchant, &w.asset, &MAX, &NOW, &w.id,),
        Err(Ok(Error::MandateExpired))
    );

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
fn canonical_timelock_upgrades_registry_at_same_address_and_preserves_state() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(NOW);

    let proposer = Address::generate(&env);
    let pauser = Address::generate(&env);
    let user = Address::generate(&env);
    let agent = Address::generate(&env);
    let merchant = Address::generate(&env);
    let asset = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    let mandate_id = BytesN::from_array(&env, &[11; 32]);

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
            asset.clone(),
        ),
    );
    let timelock_client = TimelockControllerClient::new(&env, &timelock);
    let registry_client = MandateRegistryClient::new(&env, &registry);
    StellarAssetClient::new(&env, &asset).mint(&user, &FUNDED);
    TokenClient::new(&env, &asset).approve(&user, &registry, &FUNDED, &100_000);
    registry_client.register_mandate(&user, &agent, &merchant, &asset, &MAX, &EXPIRY, &mandate_id);
    registry_client.execute_payment(&mandate_id, &SPEND, &0);
    let mandate_before = registry_client.get_mandate(&mandate_id);

    let wasm_hash = env.deployer().upload_contract_wasm(replacement_wasm(&env));
    let predecessor = BytesN::from_array(&env, &[0; 32]);
    let salt = BytesN::from_array(&env, &[12; 32]);
    let args: Vec<Val> = vec![
        &env,
        wasm_hash.clone().into_val(&env),
        timelock.clone().into_val(&env),
    ];
    let operation_id = timelock_client.schedule(
        &registry,
        &Symbol::new(&env, "upgrade"),
        &args,
        &predecessor,
        &salt,
        &10,
        &proposer,
    );

    assert!(timelock_client
        .try_execute(
            &registry,
            &Symbol::new(&env, "upgrade"),
            &args,
            &predecessor,
            &salt,
            &None,
        )
        .is_err());
    env.ledger().with_mut(|ledger| ledger.sequence_number += 10);
    timelock_client.execute(
        &registry,
        &Symbol::new(&env, "upgrade"),
        &args,
        &predecessor,
        &salt,
        &None,
    );
    assert_eq!(
        timelock_client.get_operation_state(&operation_id),
        OperationState::Done
    );

    let sum: u64 = env.invoke_contract(
        &registry,
        &symbol_short!("add"),
        (2_u64, 3_u64).into_val(&env),
    );
    assert_eq!(sum, 5);

    let (admin, schema, allowed, mandate_after) = env.as_contract(&registry, || {
        (
            stellar_access::access_control::get_admin(&env),
            stellar_contract_utils::upgradeable::get_schema_version(&env),
            crate::storage::is_asset_allowed(&env, &asset),
            crate::storage::get_mandate(&env, mandate_id.clone()).unwrap(),
        )
    });
    assert_eq!(admin, Some(timelock));
    assert_eq!(schema, SCHEMA_VERSION);
    assert!(allowed);
    assert_eq!(mandate_after, mandate_before);
}

#[test]
fn duplicate_unknown_overspend_expiry_revocation_scope_and_replay_fail() {
    let w = setup();
    let c = w.client();
    w.register();

    for invalid_amount in [0, -1] {
        assert_eq!(
            c.try_validate_mandate(&w.id, &invalid_amount, &w.merchant),
            Err(Ok(Error::InvalidAmount))
        );
        assert_eq!(
            c.try_execute_payment(&w.id, &invalid_amount, &0),
            Err(Ok(Error::InvalidAmount))
        );
    }
    assert_eq!(
        (c.get_mandate(&w.id).spent, c.get_mandate(&w.id).seq),
        (0, 0)
    );

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
