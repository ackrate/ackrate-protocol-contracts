use simple_mandate_registry::{MandateRegistry, MandateRegistryClient, Status};
use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, Ledger, MockAuth, MockAuthInvoke},
    token::{StellarAssetClient, TokenClient},
    Address, BytesN, Env, IntoVal,
};

use crate::{
    Error, ExecutionRequest, InstalledPolicy, ReferenceMandateExtension,
    ReferenceMandateExtensionClient, INTERFACE_VERSION, MAX_POLICY_LIFETIME_SECS,
};

const NOW: u64 = 1_800_000_000;
const NETWORK: [u8; 32] = [7; 32];
const MANDATE_MAX: i128 = 1_000;
const PER_PAYMENT_MAX: i128 = 250;
const FUNDED: i128 = 5_000;

struct World {
    env: Env,
    extension: Address,
    registry: Address,
    user: Address,
    executor: Address,
    merchant: Address,
    asset: Address,
    mandate_id: BytesN<32>,
}

impl World {
    fn new(expiry: u64) -> Self {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set_timestamp(NOW);
        env.ledger().set_network_id(NETWORK);

        let user = Address::generate(&env);
        let executor = Address::generate(&env);
        let merchant = Address::generate(&env);
        let extension = env.register(ReferenceMandateExtension, ());
        let registry = env.register(MandateRegistry, (Address::generate(&env),));
        let asset = env
            .register_stellar_asset_contract_v2(Address::generate(&env))
            .address();
        let mandate_id = BytesN::from_array(&env, &[1; 32]);

        MandateRegistryClient::new(&env, &registry).register_mandate(
            &user,
            &extension,
            &merchant,
            &asset,
            &MANDATE_MAX,
            &expiry,
            &mandate_id,
        );
        StellarAssetClient::new(&env, &asset).mint(&user, &FUNDED);
        TokenClient::new(&env, &asset).approve(&user, &registry, &MANDATE_MAX, &100_000);

        Self {
            env,
            extension,
            registry,
            user,
            executor,
            merchant,
            asset,
            mandate_id,
        }
    }

    fn standard() -> Self {
        Self::new(NOW + 86_400)
    }

    fn client(&self) -> ReferenceMandateExtensionClient<'_> {
        ReferenceMandateExtensionClient::new(&self.env, &self.extension)
    }

    fn registry_client(&self) -> MandateRegistryClient<'_> {
        MandateRegistryClient::new(&self.env, &self.registry)
    }

    fn install(&self) -> InstalledPolicy {
        self.client().install_policy(
            &self.registry,
            &self.mandate_id,
            &self.executor,
            &PER_PAYMENT_MAX,
            &NOW,
        )
    }

    fn request(
        &self,
        policy: &InstalledPolicy,
        nonce: u8,
        amount: i128,
        expected_seq: u32,
    ) -> ExecutionRequest {
        ExecutionRequest {
            version: INTERFACE_VERSION,
            network_id: BytesN::from_array(&self.env, &NETWORK),
            registry: self.registry.clone(),
            extension: self.extension.clone(),
            mandate_id: self.mandate_id.clone(),
            amount,
            expected_seq,
            nonce: BytesN::from_array(&self.env, &[nonce; 32]),
            valid_after: NOW - 1,
            valid_before: NOW + 600,
            policy_hash: policy.policy_hash.clone(),
        }
    }

    fn execute_with_executor_auth(&self, request: &ExecutionRequest) {
        self.env.set_auths(&[]);
        self.client()
            .mock_auths(&[MockAuth {
                address: &self.executor,
                invoke: &MockAuthInvoke {
                    contract: &self.extension,
                    fn_name: "execute",
                    args: (request.clone(),).into_val(&self.env),
                    sub_invokes: &[],
                },
            }])
            .execute(request);
    }
}

#[test]
fn immutable_policy_routes_through_real_registry_without_token_capability() {
    let world = World::standard();
    let policy = world.install();
    assert_eq!(
        world.client().policy_hash(&policy.binding),
        policy.policy_hash
    );
    assert_eq!(
        world
            .client()
            .get_policy(&world.registry, &world.mandate_id),
        policy
    );

    let request = world.request(&policy, 2, 125, 0);
    world.execute_with_executor_auth(&request);

    assert_eq!(
        TokenClient::new(&world.env, &world.asset).balance(&world.merchant),
        125
    );
    assert_eq!(
        TokenClient::new(&world.env, &world.asset).allowance(&world.user, &world.registry),
        MANDATE_MAX - 125
    );
    assert_eq!(
        TokenClient::new(&world.env, &world.asset).allowance(&world.user, &world.extension),
        0
    );
    let mandate = world.registry_client().get_mandate(&world.mandate_id);
    assert_eq!((mandate.spent, mandate.seq), (125, 1));
    assert!(world
        .client()
        .is_nonce_consumed(&policy.policy_hash, &request.nonce));
}

#[test]
fn install_requires_mandate_user_and_cannot_be_replaced() {
    let world = World::standard();
    world.env.set_auths(&[]);
    assert!(world
        .client()
        .try_install_policy(
            &world.registry,
            &world.mandate_id,
            &world.executor,
            &PER_PAYMENT_MAX,
            &NOW,
        )
        .is_err());

    world.env.mock_all_auths();
    let installed = world.install();
    assert_eq!(
        world.client().try_install_policy(
            &world.registry,
            &world.mandate_id,
            &Address::generate(&world.env),
            &1,
            &NOW,
        ),
        Err(Ok(Error::PolicyAlreadyInstalled))
    );
    assert_eq!(
        world
            .client()
            .get_policy(&world.registry, &world.mandate_id),
        installed
    );
}

#[test]
fn install_rejects_invalid_or_overlong_policy() {
    let world = World::standard();
    assert_eq!(
        world.client().try_install_policy(
            &world.registry,
            &world.mandate_id,
            &world.executor,
            &(MANDATE_MAX + 1),
            &NOW,
        ),
        Err(Ok(Error::InvalidAmount))
    );

    let overlong = World::new(NOW + MAX_POLICY_LIFETIME_SECS + 1);
    assert_eq!(
        overlong.client().try_install_policy(
            &overlong.registry,
            &overlong.mandate_id,
            &overlong.executor,
            &PER_PAYMENT_MAX,
            &NOW,
        ),
        Err(Ok(Error::PolicyTooLong))
    );

    let boundary = World::new(NOW + MAX_POLICY_LIFETIME_SECS);
    boundary.install();
}

#[test]
fn typed_request_fails_closed_on_every_common_binding() {
    let world = World::standard();
    let policy = world.install();

    let mut wrong_version = world.request(&policy, 3, 100, 0);
    wrong_version.version += 1;
    assert_eq!(
        world.client().try_execute(&wrong_version),
        Err(Ok(Error::UnsupportedVersion))
    );

    let mut wrong_network = world.request(&policy, 4, 100, 0);
    wrong_network.network_id = BytesN::from_array(&world.env, &[99; 32]);
    assert_eq!(
        world.client().try_execute(&wrong_network),
        Err(Ok(Error::WrongNetwork))
    );

    let mut wrong_extension = world.request(&policy, 5, 100, 0);
    wrong_extension.extension = Address::generate(&world.env);
    assert_eq!(
        world.client().try_execute(&wrong_extension),
        Err(Ok(Error::MandateMismatch))
    );

    let mut wrong_hash = world.request(&policy, 6, 100, 0);
    wrong_hash.policy_hash = BytesN::from_array(&world.env, &[88; 32]);
    assert_eq!(
        world.client().try_execute(&wrong_hash),
        Err(Ok(Error::PolicyHashMismatch))
    );

    let mut stale = world.request(&policy, 7, 100, 0);
    stale.valid_before = NOW;
    assert_eq!(
        world.client().try_execute(&stale),
        Err(Ok(Error::InvalidWindow))
    );
}

#[test]
fn payment_cap_executor_auth_and_nonce_replay_are_enforced() {
    let world = World::standard();
    let policy = world.install();

    let too_large = world.request(&policy, 8, PER_PAYMENT_MAX + 1, 0);
    assert_eq!(
        world.client().try_execute(&too_large),
        Err(Ok(Error::PaymentCapExceeded))
    );

    let request = world.request(&policy, 9, 100, 0);
    world.env.set_auths(&[]);
    assert!(world.client().try_execute(&request).is_err());

    world.env.mock_all_auths();
    world.client().execute(&request);
    assert_eq!(world.client().try_execute(&request), Err(Ok(Error::Replay)));
    assert_eq!(
        TokenClient::new(&world.env, &world.asset).balance(&world.merchant),
        100
    );
}

#[test]
fn registry_pause_and_stale_sequence_roll_back_extension_nonce() {
    let world = World::standard();
    let policy = world.install();

    world.registry_client().pause();
    let paused = world.request(&policy, 10, 100, 0);
    assert!(world.client().try_execute(&paused).is_err());
    assert!(!world
        .client()
        .is_nonce_consumed(&policy.policy_hash, &paused.nonce));
    assert_eq!(
        TokenClient::new(&world.env, &world.asset).balance(&world.merchant),
        0
    );

    world.registry_client().unpause();
    let first = world.request(&policy, 11, 100, 0);
    world.client().execute(&first);

    let stale = world.request(&policy, 12, 100, 0);
    assert!(world.client().try_execute(&stale).is_err());
    assert!(!world
        .client()
        .is_nonce_consumed(&policy.policy_hash, &stale.nonce));
    assert_eq!(
        world.registry_client().get_mandate(&world.mandate_id).seq,
        1
    );
}

#[test]
fn token_failure_rolls_back_registry_and_extension_state() {
    let world = World::standard();
    let policy = world.install();
    TokenClient::new(&world.env, &world.asset).approve(&world.user, &world.registry, &0, &100_000);
    let request = world.request(&policy, 13, 100, 0);

    assert!(world.client().try_execute(&request).is_err());
    assert!(!world
        .client()
        .is_nonce_consumed(&policy.policy_hash, &request.nonce));
    let mandate = world.registry_client().get_mandate(&world.mandate_id);
    assert_eq!((mandate.spent, mandate.seq), (0, 0));
}

#[test]
fn revoked_mandate_fails_closed_before_extension_consumption() {
    let world = World::standard();
    let policy = world.install();
    world.registry_client().revoke_mandate(&world.mandate_id);
    let request = world.request(&policy, 14, 100, 0);

    assert_eq!(
        world.client().try_execute(&request),
        Err(Ok(Error::MandateInactive))
    );
    assert!(!world
        .client()
        .is_nonce_consumed(&policy.policy_hash, &request.nonce));
    assert_eq!(
        world
            .registry_client()
            .get_mandate(&world.mandate_id)
            .status,
        Status::Revoked
    );
}

const TOKEN_EXTENSION: u32 = 0;
const TOKEN_REQUEST: u32 = 1;
const TOKEN_ATTEMPTED: u32 = 2;
const TOKEN_REJECTED: u32 = 3;

#[contract]
struct ReentrantToken;

#[contractimpl]
impl ReentrantToken {
    pub fn configure(env: Env, extension: Address, request: ExecutionRequest) {
        env.storage().instance().set(&TOKEN_EXTENSION, &extension);
        env.storage().instance().set(&TOKEN_REQUEST, &request);
        env.storage().instance().set(&TOKEN_ATTEMPTED, &false);
        env.storage().instance().set(&TOKEN_REJECTED, &false);
    }

    pub fn attempted(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&TOKEN_ATTEMPTED)
            .unwrap_or(false)
    }

    pub fn rejected(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&TOKEN_REJECTED)
            .unwrap_or(false)
    }

    pub fn transfer_from(env: Env, _spender: Address, _from: Address, _to: Address, _amount: i128) {
        env.storage().instance().set(&TOKEN_ATTEMPTED, &true);
        let extension: Address = env.storage().instance().get(&TOKEN_EXTENSION).unwrap();
        let request: ExecutionRequest = env.storage().instance().get(&TOKEN_REQUEST).unwrap();
        let result = ReferenceMandateExtensionClient::new(&env, &extension).try_execute(&request);
        env.storage()
            .instance()
            .set(&TOKEN_REJECTED, &result.is_err());
    }

    pub fn balance(_env: Env, _address: Address) -> i128 {
        0
    }
}

#[test]
fn reentrant_token_cannot_consume_a_second_extension_request() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(NOW);
    env.ledger().set_network_id(NETWORK);

    let user = Address::generate(&env);
    let executor = Address::generate(&env);
    let merchant = Address::generate(&env);
    let extension = env.register(ReferenceMandateExtension, ());
    let registry = env.register(MandateRegistry, (Address::generate(&env),));
    let token = env.register(ReentrantToken, ());
    let mandate_id = BytesN::from_array(&env, &[41; 32]);
    let registry_client = MandateRegistryClient::new(&env, &registry);
    registry_client.register_mandate(
        &user,
        &extension,
        &merchant,
        &token,
        &MANDATE_MAX,
        &(NOW + 86_400),
        &mandate_id,
    );

    let extension_client = ReferenceMandateExtensionClient::new(&env, &extension);
    let policy =
        extension_client.install_policy(&registry, &mandate_id, &executor, &PER_PAYMENT_MAX, &NOW);
    let make_request = |nonce: u8, seq: u32| ExecutionRequest {
        version: INTERFACE_VERSION,
        network_id: BytesN::from_array(&env, &NETWORK),
        registry: registry.clone(),
        extension: extension.clone(),
        mandate_id: mandate_id.clone(),
        amount: 100,
        expected_seq: seq,
        nonce: BytesN::from_array(&env, &[nonce; 32]),
        valid_after: NOW - 1,
        valid_before: NOW + 600,
        policy_hash: policy.policy_hash.clone(),
    };
    let outer = make_request(42, 0);
    let inner = make_request(43, 1);
    ReentrantTokenClient::new(&env, &token).configure(&extension, &inner);

    extension_client.execute(&outer);

    assert!(ReentrantTokenClient::new(&env, &token).attempted());
    assert!(ReentrantTokenClient::new(&env, &token).rejected());
    assert!(!extension_client.is_nonce_consumed(&policy.policy_hash, &inner.nonce));
    let mandate = registry_client.get_mandate(&mandate_id);
    assert_eq!((mandate.spent, mandate.seq), (100, 1));
}

#[test]
fn explicit_extension_lock_rejects_a_second_in_flight_request() {
    let world = World::standard();
    let policy = world.install();
    let request = world.request(&policy, 44, 100, 0);

    world.env.as_contract(&world.extension, || {
        crate::storage::set_executing(&world.env, &world.registry, &world.mandate_id);
    });
    assert_eq!(
        world.client().try_execute(&request),
        Err(Ok(Error::ReentrantExecution))
    );
    assert!(!world
        .client()
        .is_nonce_consumed(&policy.policy_hash, &request.nonce));
}
