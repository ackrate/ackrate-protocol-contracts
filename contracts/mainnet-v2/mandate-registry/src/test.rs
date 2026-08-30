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

    pub fn propose_admin(env: Env, registry: Address, new_admin: Address) {
        MandateRegistryClient::new(&env, &registry).propose_admin(&new_admin);
    }

    pub fn accept_admin(env: Env, registry: Address) {
        MandateRegistryClient::new(&env, &registry).accept_admin();
    }

    pub fn set_asset_allowed(env: Env, registry: Address, asset: Address, allowed: bool) {
        MandateRegistryClient::new(&env, &registry).set_asset_allowed(&asset, &allowed);
    }

    pub fn set_asset_allowed_error(
        env: Env,
        registry: Address,
        asset: Address,
        allowed: bool,
    ) -> u32 {
        match MandateRegistryClient::new(&env, &registry).try_set_asset_allowed(&asset, &allowed) {
            Ok(Ok(())) => OK,
            Err(Ok(error)) => error as u32,
            _ => HOST_FAILURE,
        }
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
    vc_hash: BytesN<32>,
    id: BytesN<32>,
}

fn setup() -> World {
    let env = Env::default();
    env.ledger().set_timestamp(NOW);

    let admin = env.register(Principal, ());
    let asset_admin = env.register(Principal, ());
    let asset = env
        .register_stellar_asset_contract_v2(asset_admin.clone())
        .address();
    let contract = env.register(MandateRegistry, (admin.clone(), asset.clone()));
    let user = env.register(Principal, ());
    let agent = env.register(Principal, ());
    let merchant = Address::generate(&env);

    PrincipalClient::new(&env, &asset_admin).mint(&asset, &user, &FUNDED);
    PrincipalClient::new(&env, &user).approve(&asset, &contract, &FUNDED, &100_000);

    let vc_hash = BytesN::from_array(&env, &[1u8; 32]);
    let id = MandateRegistryClient::new(&env, &contract)
        .derive_mandate_id(&user, &agent, &merchant, &asset, &MAX, &EXPIRY, &vc_hash);
    World {
        env,
        contract,
        admin,
        user,
        agent,
        merchant,
        asset,
        vc_hash,
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
            &self.vc_hash,
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
            &self.vc_hash,
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
    fn propose_admin(&self, new_admin: &Address) {
        self.principal(&self.admin)
            .propose_admin(&self.contract, new_admin);
    }
    fn accept_admin(&self, candidate: &Address) {
        self.principal(candidate).accept_admin(&self.contract);
    }
    fn set_asset_allowed(&self, asset: &Address, allowed: bool) {
        self.principal(&self.admin)
            .set_asset_allowed(&self.contract, asset, &allowed);
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

    // validate_mandate (non-value-moving preview)
    c.validate_mandate(&w.id, &SPEND, &0, &w.merchant, &w.asset);

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
    assert_eq!(w.client().get_schema_version(), 2);
    assert!(w.client().is_asset_allowed(&w.asset));
    assert_eq!(
        w.env.as_contract(&w.contract, || {
            crate::storage::get_schema_version(&w.env)
        }),
        Some(2)
    );
}

#[test]
fn missing_schema_blocks_mandates_but_preserves_admin_recovery() {
    let w = setup();
    w.env.as_contract(&w.contract, || {
        w.env
            .storage()
            .instance()
            .remove(&crate::storage::DataKey::SchemaVersion);
    });

    assert_eq!(w.register_error(EXPIRY), Error::InvalidState as u32);
    assert_eq!(
        w.client().try_get_schema_version(),
        Err(Ok(Error::InvalidState))
    );
    w.pause();
    assert!(w.client().is_paused());
}

#[test]
fn predecessor_schema_fails_closed_before_state_access() {
    let w = setup();
    w.register();
    let legacy_mandate = w.client().get_mandate(&w.id);
    let legacy_id = w.vc_hash.clone();
    w.env.as_contract(&w.contract, || {
        crate::storage::set_mandate(&w.env, &legacy_id, &legacy_mandate);
        w.env
            .storage()
            .instance()
            .set(&crate::storage::DataKey::SchemaVersion, &1_u32);
    });

    assert_eq!(w.register_error(EXPIRY + 1), Error::InvalidState as u32);
    assert_eq!(
        w.client().try_get_mandate(&legacy_id),
        Err(Ok(Error::InvalidState))
    );
    assert_eq!(
        w.principal(&w.agent)
            .execute_error(&w.contract, &legacy_id, &SPEND, &0),
        Error::InvalidState as u32
    );
    assert_eq!(w.balance(&w.merchant), 0);

    // Administrative recovery remains available so an incompatible instance
    // can be stopped without exposing mandate state or moving value.
    w.pause();
    assert!(w.client().is_paused());
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
        asset: w.asset.clone(),
        mandate_id: w.id.clone(),
        amount: SPEND,
        sequence: 0,
    };
    assert_eq!(
        w.env.events().all().filter_by_contract(&w.contract),
        std::vec![paid.to_xdr(&w.env, &w.contract)]
    );

    let admin_world = setup();
    let candidate = admin_world.env.register(Principal, ());
    admin_world.propose_admin(&candidate);
    let proposed = crate::events::AdminTransferProposed {
        pending_admin: candidate.clone(),
    };
    assert_eq!(
        admin_world
            .env
            .events()
            .all()
            .filter_by_contract(&admin_world.contract),
        std::vec![proposed.to_xdr(&admin_world.env, &admin_world.contract)]
    );
    admin_world.accept_admin(&candidate);
    let accepted = crate::events::AdminSet {
        new_admin: candidate,
    };
    assert_eq!(
        admin_world
            .env
            .events()
            .all()
            .filter_by_contract(&admin_world.contract),
        std::vec![accepted.to_xdr(&admin_world.env, &admin_world.contract)]
    );

    let asset_world = setup();
    let second_asset = asset_world
        .env
        .register_stellar_asset_contract_v2(Address::generate(&asset_world.env))
        .address();
    asset_world.pause();
    asset_world.set_asset_allowed(&second_asset, true);
    let policy = crate::events::AssetPolicyChanged {
        asset: second_asset,
        allowed: true,
    };
    assert_eq!(
        asset_world
            .env
            .events()
            .all()
            .filter_by_contract(&asset_world.contract),
        std::vec![policy.to_xdr(&asset_world.env, &asset_world.contract)]
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
fn registration_reads_and_revocation_remain_available_while_preview_is_paused() {
    let w = setup();
    let c = w.client();
    w.pause();

    w.register();
    assert_eq!(
        c.try_validate_mandate(&w.id, &SPEND, &0, &w.merchant, &w.asset),
        Err(Ok(Error::Paused))
    );
    assert_eq!(c.get_mandate(&w.id).status, Status::Active);
    w.revoke();
    assert_eq!(c.get_mandate(&w.id).status, Status::Revoked);
}

#[test]
fn admin_rotation_transfers_control() {
    let w = setup();
    let c = w.client();
    let new_admin = w.env.register(Principal, ());
    let abandoned_candidate = Address::generate(&w.env);
    assert_eq!(c.try_accept_admin(), Err(Ok(Error::NoPendingAdmin)));
    w.propose_admin(&abandoned_candidate);
    assert_eq!(c.get_pending_admin(), Some(abandoned_candidate));
    w.propose_admin(&new_admin);
    assert_eq!(c.get_pending_admin(), Some(new_admin.clone()));
    assert_eq!(c.get_admin(), w.admin);
    assert!(c.try_accept_admin().is_err());

    // The old administrator remains able to recover from a mistaken proposal.
    w.pause();
    w.unpause();

    w.accept_admin(&new_admin);
    assert_eq!(c.get_pending_admin(), None);
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
    assert!(c.try_propose_admin(&replacement).is_err());
    assert!(c.try_upgrade(&wasm_hash).is_err());
    assert!(c.try_set_asset_allowed(&replacement, &true).is_err());
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
        w.client()
            .try_validate_mandate(&w.id, &SPEND, &0, &attacker, &w.asset),
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
    let admin = env.register(Principal, ());
    let asset = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    let contract = env.register(MandateRegistry, (admin, asset.clone()));
    let client = MandateRegistryClient::new(&env, &contract);
    let user = Address::generate(&env);
    let agent = Address::generate(&env);
    let merchant = Address::generate(&env);
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
fn exhausted_mandate_can_record_user_revocation_without_reopening_budget() {
    let w = setup();
    w.register();
    w.execute(MAX, 0);
    assert_eq!(w.client().get_mandate(&w.id).status, Status::Exhausted);

    w.revoke();
    let revoked = w.client().get_mandate(&w.id);
    assert_eq!(revoked.status, Status::Revoked);
    assert_eq!(revoked.spent, MAX);
    assert_eq!(w.execute_error(1, 1), Error::MandateRevoked as u32);
    assert_eq!(w.balance(&w.merchant), MAX);
}

#[test]
fn insufficient_allowance_blocks_payment() {
    let w = setup();
    // Within the contract's budget, but the SEP-41 allowance is the hard ceiling.
    w.approve(SPEND - 1);
    w.register();
    let before = w.client().get_mandate(&w.id);
    assert!(w
        .principal(&w.agent)
        .try_execute(&w.contract, &w.id, &SPEND, &0)
        .is_err());
    assert_eq!(w.balance(&w.merchant), 0);
    assert_eq!(w.client().get_mandate(&w.id), before);

    // The failed token call rolled the sequence and spend back, so an exact
    // retry succeeds with the original sequence after allowance is restored.
    w.approve(FUNDED);
    w.execute(SPEND, 0);
    assert_eq!(w.client().get_mandate(&w.id).seq, 1);
    assert_eq!(w.balance(&w.merchant), SPEND);
}

#[test]
fn exact_budget_token_failure_rolls_back_exhaustion() {
    let w = setup();
    w.approve(MAX - 1);
    w.register();

    assert!(w
        .principal(&w.agent)
        .try_execute(&w.contract, &w.id, &MAX, &0)
        .is_err());
    let after_failure = w.client().get_mandate(&w.id);
    assert_eq!(after_failure.spent, 0);
    assert_eq!(after_failure.seq, 0);
    assert_eq!(after_failure.status, Status::Active);
    assert_eq!(w.balance(&w.merchant), 0);

    w.approve(FUNDED);
    w.execute(MAX, 0);
    let committed = w.client().get_mandate(&w.id);
    assert_eq!(committed.spent, MAX);
    assert_eq!(committed.seq, 1);
    assert_eq!(committed.status, Status::Exhausted);
    assert_eq!(w.balance(&w.merchant), MAX);
}

#[test]
fn reviewed_asset_policy_is_enforced_on_registration_and_execution() {
    let w = setup();
    let second_admin = w.env.register(Principal, ());
    let second_asset = w
        .env
        .register_stellar_asset_contract_v2(second_admin)
        .address();
    let second_hash = BytesN::from_array(&w.env, &[22; 32]);

    assert_eq!(
        w.principal(&w.user).register_error(
            &w.contract,
            &w.agent,
            &w.merchant,
            &second_asset,
            &MAX,
            &EXPIRY,
            &second_hash,
        ),
        Error::AssetNotAllowed as u32
    );
    assert_eq!(
        w.principal(&w.admin)
            .set_asset_allowed_error(&w.contract, &second_asset, &true,),
        Error::AssetPolicyRequiresPause as u32
    );

    w.pause();
    w.set_asset_allowed(&second_asset, true);
    w.unpause();
    assert!(w.client().is_asset_allowed(&second_asset));
    let second_id = w.principal(&w.user).register(
        &w.contract,
        &w.agent,
        &w.merchant,
        &second_asset,
        &MAX,
        &EXPIRY,
        &second_hash,
    );

    w.pause();
    w.set_asset_allowed(&second_asset, false);
    w.unpause();
    assert!(!w.client().is_asset_allowed(&second_asset));
    assert_eq!(
        w.client()
            .try_validate_mandate(&second_id, &SPEND, &0, &w.merchant, &second_asset,),
        Err(Ok(Error::AssetNotAllowed))
    );
    assert_eq!(
        w.principal(&w.agent)
            .execute_error(&w.contract, &second_id, &SPEND, &0),
        Error::AssetNotAllowed as u32
    );
}

#[test]
fn mandate_lifetime_is_bounded_below_persistence_target() {
    let w = setup();
    let latest = NOW + crate::MAX_MANDATE_LIFETIME_SECONDS;
    let accepted_hash = BytesN::from_array(&w.env, &[31; 32]);
    let rejected_hash = BytesN::from_array(&w.env, &[32; 32]);

    w.principal(&w.user).register(
        &w.contract,
        &w.agent,
        &w.merchant,
        &w.asset,
        &MAX,
        &latest,
        &accepted_hash,
    );
    assert_eq!(
        w.principal(&w.user).register_error(
            &w.contract,
            &w.agent,
            &w.merchant,
            &w.asset,
            &MAX,
            &(latest + 1),
            &rejected_hash,
        ),
        Error::MandateTooLong as u32
    );
}

#[test]
fn mandate_identifier_is_bound_to_network_registry_user_and_terms() {
    let w = setup();
    let second_user = w.env.register(Principal, ());
    let shared_hash = BytesN::from_array(&w.env, &[41; 32]);

    let first_id = w.principal(&w.user).register(
        &w.contract,
        &w.agent,
        &w.merchant,
        &w.asset,
        &MAX,
        &EXPIRY,
        &shared_hash,
    );
    assert_eq!(
        first_id.to_array(),
        [
            243, 174, 137, 90, 107, 105, 192, 176, 131, 14, 11, 187, 21, 246, 132, 69, 153, 128,
            69, 173, 230, 236, 50, 98, 2, 65, 233, 1, 31, 223, 45, 250,
        ]
    );
    let second_id = w.principal(&second_user).register(
        &w.contract,
        &w.agent,
        &w.merchant,
        &w.asset,
        &MAX,
        &EXPIRY,
        &shared_hash,
    );
    assert_ne!(first_id, second_id);
    assert_eq!(w.client().get_mandate(&first_id).user, w.user);
    assert_eq!(w.client().get_mandate(&second_id).user, second_user);

    let second_registry = w
        .env
        .register(MandateRegistry, (w.admin.clone(), w.asset.clone()));
    let other_id = MandateRegistryClient::new(&w.env, &second_registry).derive_mandate_id(
        &w.user,
        &w.agent,
        &w.merchant,
        &w.asset,
        &MAX,
        &EXPIRY,
        &shared_hash,
    );
    assert_ne!(first_id, other_id);
}

#[test]
fn credential_commitment_is_idempotent_across_changed_terms() {
    let w = setup();
    let shared_hash = BytesN::from_array(&w.env, &[42; 32]);
    w.principal(&w.user).register(
        &w.contract,
        &w.agent,
        &w.merchant,
        &w.asset,
        &MAX,
        &EXPIRY,
        &shared_hash,
    );

    assert_eq!(
        w.principal(&w.user).register_error(
            &w.contract,
            &w.agent,
            &w.merchant,
            &w.asset,
            &MAX,
            &(EXPIRY + 1),
            &shared_hash,
        ),
        Error::AlreadyExists as u32
    );
}

#[test]
fn invalid_stored_invariants_fail_closed() {
    let w = setup();
    w.register();
    let mut invalid = w.client().get_mandate(&w.id);
    invalid.spent = -1;
    w.env.as_contract(&w.contract, || {
        crate::storage::set_mandate(&w.env, &w.id, &invalid);
    });

    assert_eq!(w.execute_error(SPEND, 0), Error::InvalidState as u32);
    assert_eq!(w.balance(&w.merchant), 0);
}

#[test]
fn sequence_exhaustion_is_typed_and_atomic() {
    let w = setup();
    w.register();
    let mut mandate = w.client().get_mandate(&w.id);
    mandate.seq = u32::MAX;
    w.env.as_contract(&w.contract, || {
        crate::storage::set_mandate(&w.env, &w.id, &mandate);
    });

    assert_eq!(
        w.execute_error(1, u32::MAX),
        Error::SequenceExhausted as u32
    );
    assert_eq!(w.client().get_mandate(&w.id), mandate);
    assert_eq!(w.balance(&w.merchant), 0);
}

#[test]
fn amount_and_expiry_boundaries_cover_ten_thousand_real_host_cases() {
    let w = setup();
    w.register();
    let c = w.client();

    for amount in -5_000_i128..=5_000_i128 {
        let result = c.try_validate_mandate(&w.id, &amount, &0, &w.merchant, &w.asset);
        if amount <= 0 {
            assert_eq!(result, Err(Ok(Error::InvalidAmount)));
        } else {
            assert_eq!(result, Ok(Ok(())));
        }
    }
    assert_eq!(
        c.try_validate_mandate(&w.id, &i128::MIN, &0, &w.merchant, &w.asset),
        Err(Ok(Error::InvalidAmount))
    );
    assert_eq!(
        c.try_validate_mandate(&w.id, &i128::MAX, &0, &w.merchant, &w.asset),
        Err(Ok(Error::BudgetExceeded))
    );

    w.env.ledger().set_timestamp(EXPIRY - 1);
    assert_eq!(
        c.try_validate_mandate(&w.id, &1, &0, &w.merchant, &w.asset),
        Ok(Ok(()))
    );
    w.env.ledger().set_timestamp(EXPIRY);
    assert_eq!(
        c.try_validate_mandate(&w.id, &1, &0, &w.merchant, &w.asset),
        Err(Ok(Error::MandateExpired))
    );
    w.env.ledger().set_timestamp(EXPIRY + 1);
    assert_eq!(
        c.try_validate_mandate(&w.id, &1, &0, &w.merchant, &w.asset),
        Err(Ok(Error::MandateExpired))
    );
}

#[test]
fn state_machine_runs_thousands_of_real_host_transitions() {
    const CASES: u32 = 512;

    let w = setup();
    let c = w.client();
    let mut merchant_total = 0_i128;

    for case in 0..CASES {
        let mut raw = [0_u8; 32];
        raw[..4].copy_from_slice(&case.to_be_bytes());
        raw[4] = 0xA5;
        let vc_hash = BytesN::from_array(&w.env, &raw);
        let max_amount = 17_i128 + i128::from(case % 97);
        let first = 1_i128 + i128::from(case % 7);
        let remaining = max_amount - first;
        let id = w.principal(&w.user).register(
            &w.contract,
            &w.agent,
            &w.merchant,
            &w.asset,
            &max_amount,
            &EXPIRY,
            &vc_hash,
        );

        assert_eq!(
            w.principal(&w.agent)
                .execute_error(&w.contract, &id, &first, &1),
            Error::BadSequence as u32
        );
        assert_eq!(c.get_mandate(&id).spent, 0);

        w.principal(&w.agent).execute(&w.contract, &id, &first, &0);
        merchant_total += first;
        let after_first = c.get_mandate(&id);
        assert_eq!((after_first.spent, after_first.seq), (first, 1));
        assert_eq!(w.balance(&w.merchant), merchant_total);

        assert_eq!(
            w.principal(&w.agent)
                .execute_error(&w.contract, &id, &first, &0),
            Error::BadSequence as u32
        );
        assert_eq!(c.get_mandate(&id), after_first);

        w.principal(&w.agent)
            .execute(&w.contract, &id, &remaining, &1);
        merchant_total += remaining;
        let exhausted = c.get_mandate(&id);
        assert_eq!(
            (exhausted.spent, exhausted.seq, exhausted.status.clone()),
            (max_amount, 2, Status::Exhausted)
        );
        assert_eq!(w.balance(&w.merchant), merchant_total);

        assert_eq!(
            w.principal(&w.agent)
                .execute_error(&w.contract, &id, &1, &2),
            Error::BudgetExceeded as u32
        );
        assert_eq!(c.get_mandate(&id), exhausted);
    }
}

/// Executed separately by the repository gate after the optimized artifact is
/// built. This registers the exact release bytes in the Soroban host and calls
/// them through the generated public client.
#[cfg(feature = "release-wasm-test")]
#[test]
fn optimized_release_wasm_executes_reviewed_enforcement_surface() {
    let wasm_path = std::env::var("MAINNET_V2_RELEASE_WASM")
        .expect("MAINNET_V2_RELEASE_WASM must point to the optimized artifact");
    let wasm = std::fs::read(wasm_path).expect("optimized release WASM must be readable");

    let env = Env::default();
    env.ledger().set_timestamp(NOW);
    let admin = env.register(Principal, ());
    let asset_admin = env.register(Principal, ());
    let asset = env
        .register_stellar_asset_contract_v2(asset_admin.clone())
        .address();
    let registry = env.register(wasm.as_slice(), (admin, asset.clone()));
    let user = env.register(Principal, ());
    let agent = env.register(Principal, ());
    let merchant = Address::generate(&env);
    let vc_hash = BytesN::from_array(&env, &[0xA4; 32]);
    let client = MandateRegistryClient::new(&env, &registry);

    assert_eq!(client.get_schema_version(), 2);
    assert!(client.is_asset_allowed(&asset));
    assert!(client
        .try_register_mandate(&user, &agent, &merchant, &asset, &MAX, &EXPIRY, &vc_hash,)
        .is_err());

    PrincipalClient::new(&env, &asset_admin).mint(&asset, &user, &FUNDED);
    PrincipalClient::new(&env, &user).approve(&asset, &registry, &SPEND, &100_000);
    let id = PrincipalClient::new(&env, &user).register(
        &registry, &agent, &merchant, &asset, &MAX, &EXPIRY, &vc_hash,
    );
    PrincipalClient::new(&env, &agent).execute(&registry, &id, &SPEND, &0);
    assert_eq!(
        env.events()
            .all()
            .filter_by_contract(&registry)
            .events()
            .len(),
        1
    );
    assert_eq!(TokenClient::new(&env, &asset).balance(&merchant), SPEND);
    let committed = client.get_mandate(&id);
    assert_eq!((committed.spent, committed.seq), (SPEND, 1));

    // The exact release bytes must preserve atomic rollback when the real SAC
    // allowance is exhausted.
    assert_ne!(
        PrincipalClient::new(&env, &agent).execute_error(&registry, &id, &1, &1),
        OK
    );
    assert_eq!(client.get_mandate(&id), committed);
    assert_eq!(TokenClient::new(&env, &asset).balance(&merchant), SPEND);
}
