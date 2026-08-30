//! Integration + §10 negative suite — runs in CI from commit one.
//! Each negative asserts the exact typed error (or host revert for auth); the
//! happy path asserts balances actually move through the SEP-41 token.

#![cfg(test)]

extern crate std;

use soroban_sdk::testutils::{
    storage::{Instance as _, Persistent as _},
    Address as _, Events as _, Ledger as _,
};
use soroban_sdk::token::{StellarAssetClient, TokenClient};
use soroban_sdk::{
    contract, contractimpl, symbol_short, Address, Bytes, BytesN, Env, Event as _, IntoVal,
};

use crate::{Error, MandateRegistry, MandateRegistryClient, Status};

const NOW: u64 = 1_000;
const EXPIRY: u64 = 10_000;
const MAX: i128 = 50_000_000; // 5.00 USDC
const SPEND: i128 = 10_000_000; // 1.00 USDC
const FUNDED: i128 = 1_000_000_000;

// A known-valid Protocol-21 `add(u64,u64)->u64` contract from soroban-sdk
// 22.0.11. The bytes are uploaded and executed by the real host upgrade path.
const REPLACEMENT_WASM_HEX: &str = "0061736d0100000001140460017e017e60027f7e0060027e7e017e600000020d020169013000000169015f0000030605010203030305030100100619037f01418080c0000b7f00418080c0000b7f00418080c0000b072f05066d656d6f72790200036164640003015f00060a5f5f646174615f656e6403010b5f5f686561705f6261736503020a8c02055d02017f017e024002402001a741ff0171220241c000460d00024020024106460d00420121034283908080800121010c020b20014208882101420021030c010b42002103200110808080800021010b20002001370308200020033703000b990101017f23808080800041206b2202248080808000200241106a20001082808080000240024020022802100d0020022903182100200220011082808080002002290300a70d00200020022903087c22012000540d0102400240200142ffffffffffffffff00560d00200142088642068421000c010b200110818080800021000b200241206a24808080800020000f0b00000b108480808000000b0900108580808000000b040000000b02000b004b0e636f6e7472616374737065637630000000000000000000000003616464000000000200000000000000016100000000000006000000000000000162000000000000060000000100000006001e11636f6e7472616374656e766d6574617630000000000000001500000000007b0e636f6e74726163746d65746176300000000000000005727376657200000000000006312e37342e3000000000000000000008727373646b7665720000003932312e302e312d707265766965772e312331313663333562633965303366346231623565363562356565383331616530663836616139326664000000";

const OK: u32 = 0;
const HOST_FAILURE: u32 = u32::MAX;

/// A real contract principal used to exercise contract-account authorization.
/// When this contract calls a method that requires its own address, Soroban's
/// host authorizes the nested invocation without test authorization overrides.
#[contract]
pub struct Principal;

#[contractimpl]
impl Principal {
    #[allow(clippy::too_many_arguments)]
    pub fn register(
        env: Env,
        registry: Address,
        agent: Address,
        merchant: Address,
        asset: Address,
        max_amount: i128,
        expiry: u64,
        mandate_id: BytesN<32>,
    ) -> BytesN<32> {
        MandateRegistryClient::new(&env, &registry).register_mandate(
            &env.current_contract_address(),
            &agent,
            &merchant,
            &asset,
            &max_amount,
            &expiry,
            &mandate_id,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn register_error(
        env: Env,
        registry: Address,
        agent: Address,
        merchant: Address,
        asset: Address,
        max_amount: i128,
        expiry: u64,
        mandate_id: BytesN<32>,
    ) -> u32 {
        match MandateRegistryClient::new(&env, &registry).try_register_mandate(
            &env.current_contract_address(),
            &agent,
            &merchant,
            &asset,
            &max_amount,
            &expiry,
            &mandate_id,
        ) {
            Ok(Ok(_)) => OK,
            Err(Ok(error)) => error as u32,
            _ => HOST_FAILURE,
        }
    }

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

    pub fn execute_error(
        env: Env,
        registry: Address,
        mandate_id: BytesN<32>,
        amount: i128,
        expected_seq: u32,
    ) -> u32 {
        match MandateRegistryClient::new(&env, &registry).try_execute_payment(
            &mandate_id,
            &amount,
            &expected_seq,
        ) {
            Ok(Ok(())) => OK,
            Err(Ok(error)) => error as u32,
            _ => HOST_FAILURE,
        }
    }

    pub fn revoke(env: Env, registry: Address, mandate_id: BytesN<32>) {
        MandateRegistryClient::new(&env, &registry).revoke_mandate(&mandate_id);
    }

    pub fn pause(env: Env, registry: Address) {
        MandateRegistryClient::new(&env, &registry).pause();
    }

    pub fn unpause(env: Env, registry: Address) {
        MandateRegistryClient::new(&env, &registry).unpause();
    }

    pub fn set_admin(env: Env, registry: Address, new_admin: Address) {
        MandateRegistryClient::new(&env, &registry).set_admin(&new_admin);
    }

    pub fn upgrade(env: Env, registry: Address, new_wasm_hash: BytesN<32>) {
        MandateRegistryClient::new(&env, &registry).upgrade(&new_wasm_hash);
    }

    pub fn upgrade_error(env: Env, registry: Address, new_wasm_hash: BytesN<32>) -> u32 {
        match MandateRegistryClient::new(&env, &registry).try_upgrade(&new_wasm_hash) {
            Ok(Ok(())) => OK,
            Err(Ok(error)) => error as u32,
            _ => HOST_FAILURE,
        }
    }

    pub fn mint(env: Env, asset: Address, to: Address, amount: i128) {
        StellarAssetClient::new(&env, &asset).mint(&to, &amount);
    }

    pub fn approve(
        env: Env,
        asset: Address,
        spender: Address,
        amount: i128,
        expiration_ledger: u32,
    ) {
        TokenClient::new(&env, &asset).approve(
            &env.current_contract_address(),
            &spender,
            &amount,
            &expiration_ledger,
        );
    }
}

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
    admin: Address,
    user: Address,
    agent: Address,
    merchant: Address,
    asset: Address,
    id: BytesN<32>,
}

fn setup() -> World {
    let env = Env::default();
    env.ledger().set_timestamp(NOW);

    let admin = env.register(Principal, ());
    let contract = env.register(MandateRegistry, (admin.clone(),));
    let user = env.register(Principal, ());
    let agent = env.register(Principal, ());
    let merchant = Address::generate(&env);

    let asset_admin = env.register(Principal, ());
    let asset = env
        .register_stellar_asset_contract_v2(asset_admin.clone())
        .address();

    PrincipalClient::new(&env, &asset_admin).mint(&asset, &user, &FUNDED);
    PrincipalClient::new(&env, &user).approve(&asset, &contract, &FUNDED, &100_000);

    let id = BytesN::from_array(&env, &[1u8; 32]);
    World {
        env,
        contract,
        admin,
        user,
        agent,
        merchant,
        asset,
        id,
    }
}

impl World {
    fn client(&self) -> MandateRegistryClient<'_> {
        MandateRegistryClient::new(&self.env, &self.contract)
    }
    fn principal<'a>(&'a self, address: &'a Address) -> PrincipalClient<'a> {
        PrincipalClient::new(&self.env, address)
    }
    fn register(&self) -> BytesN<32> {
        self.principal(&self.user).register(
            &self.contract,
            &self.agent,
            &self.merchant,
            &self.asset,
            &MAX,
            &EXPIRY,
            &self.id,
        )
    }
    fn register_error(&self, expiry: u64) -> u32 {
        self.principal(&self.user).register_error(
            &self.contract,
            &self.agent,
            &self.merchant,
            &self.asset,
            &MAX,
            &expiry,
            &self.id,
        )
    }
    fn execute(&self, amount: i128, expected_seq: u32) {
        self.principal(&self.agent)
            .execute(&self.contract, &self.id, &amount, &expected_seq);
    }
    fn execute_error(&self, amount: i128, expected_seq: u32) -> u32 {
        self.principal(&self.agent)
            .execute_error(&self.contract, &self.id, &amount, &expected_seq)
    }
    fn revoke(&self) {
        self.principal(&self.user).revoke(&self.contract, &self.id);
    }
    fn pause(&self) {
        self.principal(&self.admin).pause(&self.contract);
    }
    fn unpause(&self) {
        self.principal(&self.admin).unpause(&self.contract);
    }
    fn set_admin(&self, new_admin: &Address) {
        self.principal(&self.admin)
            .set_admin(&self.contract, new_admin);
    }
    fn upgrade(&self, new_wasm_hash: &BytesN<32>) {
        self.principal(&self.admin)
            .upgrade(&self.contract, new_wasm_hash);
    }
    fn upgrade_error(&self, new_wasm_hash: &BytesN<32>) -> u32 {
        self.principal(&self.admin)
            .upgrade_error(&self.contract, new_wasm_hash)
    }
    fn approve(&self, amount: i128) {
        self.principal(&self.user)
            .approve(&self.asset, &self.contract, &amount, &100_000);
    }
    fn balance(&self, who: &Address) -> i128 {
        TokenClient::new(&self.env, &self.asset).balance(who)
    }
}

// ── happy path — every method end to end ────────────────────────────────────

#[test]
fn happy_path_runs_every_method() {
    let w = setup();
    let c = w.client();

    // register
    let returned = w.register();
    assert_eq!(returned, w.id);

    // get_mandate
    let m = c.get_mandate(&w.id);
    assert_eq!(m.spent, 0);
    assert_eq!(m.max_amount, MAX);
    assert_eq!(m.seq, 0);

    // validate_mandate (read-only preflight)
    c.validate_mandate(&w.id, &SPEND, &w.merchant);

    // execute_payment — funds actually move (seq starts at 0)
    w.execute(SPEND, 0);
    assert_eq!(w.balance(&w.merchant), SPEND);
    assert_eq!(w.balance(&w.user), FUNDED - SPEND);
    assert_eq!(c.get_mandate(&w.id).spent, SPEND);
    assert_eq!(c.get_mandate(&w.id).seq, 1);

    // revoke_mandate (seq is now 1)
    w.revoke();
    assert_eq!(w.execute_error(SPEND, 1), Error::MandateRevoked as u32);
}

// ── administration + emergency stop ───────────────────────────────────────

#[test]
fn constructor_sets_admin_and_unpaused_state() {
    let w = setup();
    assert_eq!(w.client().get_admin(), w.admin);
    assert!(!w.client().is_paused());
    assert_eq!(
        w.env.as_contract(&w.contract, || {
            crate::storage::get_schema_version(&w.env)
        }),
        1
    );
}

#[test]
fn missing_pause_state_fails_closed() {
    let w = setup();
    w.env.as_contract(&w.contract, || {
        w.env
            .storage()
            .instance()
            .remove(&crate::storage::DataKey::Paused);
    });

    assert!(w.client().is_paused());
    w.register();
    assert_eq!(
        w.client().try_execute_payment(&w.id, &SPEND, &0),
        Err(Ok(Error::Paused))
    );
    assert_eq!(w.balance(&w.merchant), 0);
}

#[test]
fn contract_principals_authorize_only_their_own_nested_calls() {
    let w = setup();
    w.register();
    assert!(w.client().try_execute_payment(&w.id, &SPEND, &0).is_err());
    w.execute(SPEND, 0);
    assert_eq!(w.balance(&w.merchant), SPEND);
}

#[test]
fn typed_events_match_the_reviewed_xdr_shapes() {
    let w = setup();

    w.register();
    let registered = crate::events::MandateRegistered {
        user: w.user.clone(),
        mandate_id: w.id.clone(),
    };
    assert_eq!(
        w.env.events().all().filter_by_contract(&w.contract),
        std::vec![registered.to_xdr(&w.env, &w.contract)]
    );

    w.execute(SPEND, 0);
    let paid = crate::events::PaymentExecuted {
        merchant: w.merchant.clone(),
        mandate_id: w.id.clone(),
        amount: SPEND,
    };
    assert_eq!(
        w.env.events().all().filter_by_contract(&w.contract),
        std::vec![paid.to_xdr(&w.env, &w.contract)]
    );
}

#[test]
fn active_contract_and_mandate_ttls_reach_the_reviewed_floor() {
    let w = setup();
    w.register();

    let (instance_ttl, mandate_ttl) = w.env.as_contract(&w.contract, || {
        (
            w.env.storage().instance().get_ttl(),
            w.env
                .storage()
                .persistent()
                .get_ttl(&crate::storage::DataKey::Mandate(w.id.clone())),
        )
    });
    assert!(instance_ttl >= crate::storage::TTL_EXTEND - 1);
    assert!(mandate_ttl >= crate::storage::TTL_EXTEND - 1);
}

#[test]
fn admin_can_pause_and_unpause_idempotently() {
    let w = setup();
    let c = w.client();
    w.pause();
    w.pause();
    assert!(c.is_paused());
    w.unpause();
    w.unpause();
    assert!(!c.is_paused());
}

#[test]
fn pause_blocks_payment_without_changing_mandate_state() {
    let w = setup();
    let c = w.client();
    w.register();
    w.pause();

    assert_eq!(
        c.try_execute_payment(&w.id, &SPEND, &0),
        Err(Ok(Error::Paused))
    );
    assert_eq!(c.get_mandate(&w.id).spent, 0);
    assert_eq!(c.get_mandate(&w.id).seq, 0);
    assert_eq!(w.balance(&w.merchant), 0);

    w.unpause();
    w.execute(SPEND, 0);
    assert_eq!(w.balance(&w.merchant), SPEND);
}

#[test]
fn registration_validation_reads_and_revocation_remain_available_while_paused() {
    let w = setup();
    let c = w.client();
    w.pause();

    w.register();
    c.validate_mandate(&w.id, &SPEND, &w.merchant);
    assert_eq!(c.get_mandate(&w.id).status, Status::Active);
    w.revoke();
    assert_eq!(c.get_mandate(&w.id).status, Status::Revoked);
}

#[test]
fn admin_rotation_transfers_control() {
    let w = setup();
    let c = w.client();
    let new_admin = w.env.register(Principal, ());
    w.set_admin(&new_admin);
    assert_eq!(c.get_admin(), new_admin);

    assert!(c.try_pause().is_err());

    w.principal(&new_admin).pause(&w.contract);
    assert!(c.is_paused());
}

#[test]
fn admin_methods_require_authorization() {
    let w = setup();
    let c = w.client();
    let replacement = Address::generate(&w.env);
    let wasm_hash = BytesN::from_array(&w.env, &[42u8; 32]);
    assert!(c.try_pause().is_err());
    assert!(c.try_unpause().is_err());
    assert!(c.try_set_admin(&replacement).is_err());
    assert!(c.try_upgrade(&wasm_hash).is_err());
    assert!(!c.is_paused());
    assert_eq!(c.get_admin(), w.admin);
}

#[test]
fn upgrade_requires_pause_without_changing_state() {
    let w = setup();
    let c = w.client();
    w.register();
    let mandate_before = c.get_mandate(&w.id);
    let wasm_hash = w
        .env
        .deployer()
        .upload_contract_wasm(replacement_wasm(&w.env));

    assert_eq!(
        w.upgrade_error(&wasm_hash),
        Error::UpgradeRequiresPause as u32
    );
    assert!(!c.is_paused());
    assert_eq!(c.get_admin(), w.admin);
    assert_eq!(c.get_mandate(&w.id), mandate_before);
}

#[test]
fn paused_admin_upgrade_replaces_wasm_at_same_address_and_preserves_storage() {
    let w = setup();
    let c = w.client();
    w.register();
    w.execute(SPEND, 0);
    let mandate_before = c.get_mandate(&w.id);
    let contract_before = w.contract.clone();

    let wasm_hash = w
        .env
        .deployer()
        .upload_contract_wasm(replacement_wasm(&w.env));
    w.pause();
    w.upgrade(&wasm_hash);

    let sum: u64 = w.env.invoke_contract(
        &contract_before,
        &symbol_short!("add"),
        (2_u64, 3_u64).into_val(&w.env),
    );
    assert_eq!(sum, 5);
    assert_eq!(w.contract, contract_before);

    let (admin, paused, mandate_after) = w.env.as_contract(&w.contract, || {
        (
            crate::storage::get_admin(&w.env),
            crate::storage::is_paused(&w.env),
            crate::storage::get_mandate(&w.env, w.id.clone()).unwrap(),
        )
    });
    assert_eq!(admin, w.admin);
    assert!(paused);
    assert_eq!(mandate_after, mandate_before);
}

#[test]
fn property_spent_equals_transferred() {
    let w = setup();
    let c = w.client();
    w.register();
    w.execute(SPEND, 0);
    w.execute(SPEND, 1);
    assert_eq!(c.get_mandate(&w.id).spent, 2 * SPEND);
    assert_eq!(w.balance(&w.merchant), 2 * SPEND);
}

// ── §10 negative suite ──────────────────────────────────────────────────────

#[test]
fn duplicate_register_rejected() {
    let w = setup();
    w.register();
    assert_eq!(w.register_error(EXPIRY), Error::AlreadyExists as u32);
}

#[test]
fn unknown_mandate_not_found() {
    let w = setup();
    let unknown = BytesN::from_array(&w.env, &[9u8; 32]);
    assert_eq!(
        w.client().try_get_mandate(&unknown),
        Err(Ok(Error::NotFound))
    );
    assert_eq!(
        w.client().try_execute_payment(&unknown, &SPEND, &0),
        Err(Ok(Error::NotFound))
    );
}

#[test]
fn overspend_single_rejected() {
    let w = setup();
    w.register();
    assert_eq!(w.execute_error(MAX + 1, 0), Error::BudgetExceeded as u32);
    assert_eq!(w.balance(&w.merchant), 0);
}

#[test]
fn overspend_cumulative_rejected() {
    let w = setup();
    w.register();
    w.execute(MAX - SPEND, 0);
    assert_eq!(w.execute_error(SPEND + 1, 1), Error::BudgetExceeded as u32);
    assert_eq!(w.balance(&w.merchant), MAX - SPEND);
}

#[test]
fn expired_mandate_rejected() {
    let w = setup();
    w.register();
    w.env.ledger().set_timestamp(EXPIRY + 1);
    assert_eq!(w.execute_error(SPEND, 0), Error::MandateExpired as u32);
    assert_eq!(w.balance(&w.merchant), 0);
}

#[test]
fn revoked_mandate_rejected() {
    let w = setup();
    w.register();
    w.revoke();
    assert_eq!(w.execute_error(SPEND, 0), Error::MandateRevoked as u32);
}

#[test]
fn out_of_scope_merchant_rejected() {
    let w = setup();
    w.register();
    let attacker = Address::generate(&w.env);
    assert_eq!(
        w.client().try_validate_mandate(&w.id, &SPEND, &attacker),
        Err(Ok(Error::MerchantOutOfScope))
    );
}

#[test]
fn zero_amount_rejected() {
    let w = setup();
    w.register();
    assert_eq!(w.execute_error(0, 0), Error::InvalidAmount as u32);
}

#[test]
fn register_with_past_expiry_rejected() {
    let w = setup();
    assert_eq!(w.register_error(NOW - 1), Error::MandateExpired as u32);
}

// ── replay / sequence (§4.4) ────────────────────────────────────────────────

#[test]
fn replay_stale_seq_rejected() {
    let w = setup();
    w.register();
    w.execute(SPEND, 0); // consumes seq 0, advances to 1
                         // Re-submitting the same (now stale) seq is a replay → rejected.
    assert_eq!(w.execute_error(SPEND, 0), Error::BadSequence as u32);
    assert_eq!(w.balance(&w.merchant), SPEND); // moved exactly once
}

#[test]
fn out_of_order_seq_rejected() {
    let w = setup();
    w.register();
    // Current seq is 0; a future/out-of-order seq is rejected.
    assert_eq!(w.execute_error(SPEND, 7), Error::BadSequence as u32);
    assert_eq!(w.balance(&w.merchant), 0);
}

// ── auth suite (the security-central cases) ─────────────────────────────────
// Missing or forged authorization must revert at the host layer.

#[test]
fn register_requires_user_auth() {
    let env = Env::default();
    env.ledger().set_timestamp(NOW);
    let contract = env.register(MandateRegistry, (env.register(Principal, ()),));
    let client = MandateRegistryClient::new(&env, &contract);
    let user = Address::generate(&env);
    let agent = Address::generate(&env);
    let merchant = Address::generate(&env);
    let asset = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    let id = BytesN::from_array(&env, &[2u8; 32]);

    let r = client.try_register_mandate(&user, &agent, &merchant, &asset, &MAX, &EXPIRY, &id);
    assert!(r.is_err());
}

#[test]
fn execute_requires_agent_auth() {
    let w = setup();
    w.register();
    let r = w.client().try_execute_payment(&w.id, &SPEND, &0);
    assert!(r.is_err());
    assert_eq!(w.balance(&w.merchant), 0); // no funds moved without agent auth
}

#[test]
fn revoke_requires_user_auth() {
    let w = setup();
    w.register();
    assert!(w.client().try_revoke_mandate(&w.id).is_err());
    assert_eq!(w.client().get_mandate(&w.id).status, Status::Active); // still active
}

// ── state-machine + defense-in-depth ────────────────────────────────────────

#[test]
fn exhausted_status_then_rejected() {
    let w = setup();
    let c = w.client();
    w.register();
    w.execute(MAX, 0); // spends the whole budget
    assert_eq!(c.get_mandate(&w.id).status, Status::Exhausted);
    assert_eq!(w.execute_error(1, 1), Error::BudgetExceeded as u32);
}

#[test]
fn insufficient_allowance_blocks_payment() {
    let w = setup();
    // Within the contract's budget, but the SEP-41 allowance is the hard ceiling.
    w.approve(SPEND - 1);
    w.register();
    assert!(w
        .principal(&w.agent)
        .try_execute(&w.contract, &w.id, &SPEND, &0)
        .is_err());
    assert_eq!(w.balance(&w.merchant), 0);
}
