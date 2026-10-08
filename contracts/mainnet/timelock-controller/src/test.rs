extern crate std;

use crate::{Action, TimelockControllerClient};
use soroban_sdk::{
    contract, contractimpl, symbol_short,
    testutils::{EnvTestConfig, Ledger, MockAuth, MockAuthInvoke},
    vec, Address, BytesN, Env, IntoVal, Symbol, TryFromVal, Val, Vec,
};
use stellar_governance::timelock::OperationState;

// Exact source-account invocation trees, with host authorization enabled.
// The network demonstration separately proves transaction-level 2-of-3 signatures.
trait ExactSourceAuth {
    fn authorize(&self, auths: &[MockAuth]);
}
impl ExactSourceAuth for Env {
    fn authorize(&self, auths: &[MockAuth]) {
        if let Some(auth) = auths.first() {
            self.host()
                .set_source_account(auth.address.try_into().unwrap())
                .unwrap();
        }
        self.set_auths(
            &auths
                .iter()
                .map(|auth| soroban_sdk::xdr::SorobanAuthorizationEntry {
                    credentials: soroban_sdk::xdr::SorobanCredentials::SourceAccount,
                    root_invocation: auth.invoke.into(),
                })
                .collect::<std::vec::Vec<_>>(),
        );
    }
}

const HELLO: &[u8] =
    include_bytes!("../../../admin/target/wasm32v1-none/release/ackrate_admin_hello.wasm");
const NEXT: &[u8] = HELLO;
const HELPER: &[u8] =
    include_bytes!("../../../admin/target/wasm32v1-none/release/ackrate_timelock_controller.wasm");

fn env() -> Env {
    let e = Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    });
    e.cost_estimate().budget().reset_unlimited();
    e.ledger().with_mut(|l| l.sequence_number = 1000);
    e
}
fn gov(e: &Env) -> Address {
    Address::from_str(
        e,
        "GCIURCX7JHEKQLRTW6RDZU7OJUVCDM7WWNQPIKRERIHQOHSLW7UY7TXG",
    )
}
fn zero(e: &Env) -> BytesN<32> {
    BytesN::from_array(e, &[0; 32])
}
fn salt(e: &Env, n: u8) -> BytesN<32> {
    BytesN::from_array(e, &[n; 32])
}
fn invoke<T: TryFromVal<Env, Val>>(e: &Env, target: &Address, method: &str, args: Vec<Val>) -> T {
    e.invoke_contract(target, &Symbol::new(e, method), args)
}
fn auth(e: &Env, target: &Address, method: &str, args: Vec<Val>) {
    e.authorize(&[MockAuth {
        address: &gov(e),
        invoke: &MockAuthInvoke {
            contract: target,
            fn_name: method,
            args,
            sub_invokes: &[],
        },
    }]);
}
fn pair(e: &Env) -> (Address, Address) {
    let authority = gov(e);
    let target = e.register(HELLO, (authority.clone(),));
    adopt(e, target)
}

#[test]
fn bootstrap_governance_handoff_requires_recipient_and_freezes_after_adoption() {
    let e = env();
    let initial = gov(&e);
    let successor = Address::from_str(
        &e,
        "GD3UEYYZRU53VBAVGEKR6HYQ3USQ3FEBT5BLOYEX356EFOM5SR5774GW",
    );
    let target = e.register(HELLO, (initial.clone(),));
    let helper = e.register(HELPER, (target.clone(), initial.clone(), 10u32));
    let c = TimelockControllerClient::new(&e, &helper);
    assert!(!c.is_adopted());
    assert!(c.try_propose_governance(&successor).is_err());
    auth(
        &e,
        &helper,
        "propose_governance",
        (successor.clone(),).into_val(&e),
    );
    c.propose_governance(&successor);
    e.authorize(&[]);
    assert_eq!(c.get_pending_governance(), Some(successor.clone()));
    assert!(c.try_accept_governance().is_err());
    auth(&e, &helper, "accept_governance", vec![&e]);
    assert!(c.try_accept_governance().is_err()); // old owner cannot accept for recipient
    e.authorize(&[MockAuth {
        address: &successor,
        invoke: &MockAuthInvoke {
            contract: &helper,
            fn_name: "accept_governance",
            args: vec![&e],
            sub_invokes: &[],
        },
    }]);
    c.accept_governance();
    e.authorize(&[]);
    assert_eq!(c.get_config().governance, successor.clone());
    assert_eq!(c.get_config().initial_admin, initial.clone());
    assert_eq!(c.get_pending_governance(), None);
    auth(
        &e,
        &helper,
        "propose_governance",
        (initial.clone(),).into_val(&e),
    );
    assert!(c.try_propose_governance(&initial).is_err()); // old owner lost authority
    auth(&e, &target, "propose_admin", (helper.clone(),).into_val(&e));
    invoke::<()>(&e, &target, "propose_admin", (helper.clone(),).into_val(&e));
    e.authorize(&[MockAuth {
        address: &successor,
        invoke: &MockAuthInvoke {
            contract: &helper,
            fn_name: "accept_management",
            args: vec![&e],
            sub_invokes: &[],
        },
    }]);
    c.accept_management();
    e.authorize(&[]);
    assert!(c.is_adopted());
    assert_eq!(
        invoke::<Address>(&e, &target, "get_admin", vec![&e]),
        helper.clone()
    );
    e.authorize(&[MockAuth {
        address: &successor,
        invoke: &MockAuthInvoke {
            contract: &helper,
            fn_name: "propose_governance",
            args: (initial.clone(),).into_val(&e),
            sub_invokes: &[],
        },
    }]);
    assert_eq!(
        c.try_propose_governance(&initial),
        Err(Ok(soroban_sdk::Error::from_contract_error(104)))
    );
    // Frozen even after a delayed transfer returns the target to its original owner.
    let action = Action::TransferAdmin(initial.clone());
    let p = zero(&e);
    let s = salt(&e, 8);
    e.authorize(&[MockAuth {
        address: &successor,
        invoke: &MockAuthInvoke {
            contract: &helper,
            fn_name: "schedule",
            args: (action.clone(), p.clone(), s.clone()).into_val(&e),
            sub_invokes: &[],
        },
    }]);
    c.schedule(&action, &p, &s);
    e.authorize(&[]);
    e.ledger().with_mut(|l| l.sequence_number = 1010);
    c.execute(&action, &p, &s);
    auth(&e, &target, "accept_admin", vec![&e]);
    invoke::<()>(&e, &target, "accept_admin", vec![&e]);
    e.authorize(&[MockAuth {
        address: &successor,
        invoke: &MockAuthInvoke {
            contract: &helper,
            fn_name: "propose_governance",
            args: (initial.clone(),).into_val(&e),
            sub_invokes: &[],
        },
    }]);
    assert_eq!(
        c.try_propose_governance(&initial),
        Err(Ok(soroban_sdk::Error::from_contract_error(104)))
    );
}

fn adopt(e: &Env, target: Address) -> (Address, Address) {
    let authority = gov(e);
    let helper = e.register(HELPER, (target.clone(), authority, 10u32));
    auth(e, &target, "propose_admin", (helper.clone(),).into_val(e));
    invoke::<()>(e, &target, "propose_admin", (helper.clone(),).into_val(e));
    auth(e, &helper, "accept_management", vec![e]);
    TimelockControllerClient::new(e, &helper).accept_management();
    e.authorize(&[]);
    assert_eq!(invoke::<Address>(e, &target, "get_admin", vec![e]), helper);
    (target, helper)
}

#[test]
fn ownership_transfer_is_delayed_and_successor_must_accept() {
    let e = env();
    let (target, helper) = pair(&e);
    let c = TimelockControllerClient::new(&e, &helper);
    let successor = Address::from_str(
        &e,
        "GD3UEYYZRU53VBAVGEKR6HYQ3USQ3FEBT5BLOYEX356EFOM5SR5774GW",
    );
    let action = Action::TransferAdmin(successor.clone());
    let p = zero(&e);
    let s = salt(&e, 2);
    schedule(&e, &helper, &action, &p, &s);
    assert!(c.try_execute(&action, &p, &s).is_err());
    assert_eq!(
        invoke::<Option<Address>>(&e, &target, "get_pending_admin", vec![&e]),
        None
    );
    e.ledger().with_mut(|l| l.sequence_number = 1010);
    c.execute(&action, &p, &s);
    assert_eq!(
        invoke::<Address>(&e, &target, "get_admin", vec![&e]),
        helper.clone()
    );
    assert_eq!(
        invoke::<Option<Address>>(&e, &target, "get_pending_admin", vec![&e]),
        Some(successor.clone())
    );
    assert!(e
        .try_invoke_contract::<Val, soroban_sdk::Error>(
            &target,
            &Symbol::new(&e, "accept_admin"),
            vec![&e]
        )
        .is_err());
    e.authorize(&[MockAuth {
        address: &successor,
        invoke: &MockAuthInvoke {
            contract: &target,
            fn_name: "accept_admin",
            args: vec![&e],
            sub_invokes: &[],
        },
    }]);
    invoke::<()>(&e, &target, "accept_admin", vec![&e]);
    e.authorize(&[]);
    assert_eq!(
        invoke::<Address>(&e, &target, "get_admin", vec![&e]),
        successor
    );
    assert!(matches!(
        c.try_execute(&Action::Unpause, &p, &salt(&e, 3)),
        Err(Ok(error)) if error == soroban_sdk::Error::from_contract_error(102)
    ));
    auth(&e, &helper, "accept_management", vec![&e]);
    assert_eq!(
        c.try_accept_management(),
        Err(Ok(soroban_sdk::Error::from_contract_error(
            crate::Error::InvalidHandoff as u32,
        )))
    );
}

#[test]
fn fresh_timelock_replaces_old_instance_only_after_delayed_nomination() {
    let e = env();
    let (target, old) = pair(&e);
    let fresh = e.register(HELPER, (target.clone(), gov(&e), 10u32));
    let next = TimelockControllerClient::new(&e, &fresh);
    let previous = TimelockControllerClient::new(&e, &old);
    assert_eq!(next.get_config().initial_admin, old);
    auth(&e, &fresh, "accept_management", vec![&e]);
    assert!(next.try_accept_management().is_err());
    let action = Action::TransferAdmin(fresh.clone());
    let p = zero(&e);
    let s = salt(&e, 9);
    schedule(&e, &old, &action, &p, &s);
    assert!(previous.try_execute(&action, &p, &s).is_err());
    assert_eq!(invoke::<Address>(&e, &target, "get_admin", vec![&e]), old);
    e.ledger().with_mut(|l| l.sequence_number = 1010);
    previous.execute(&action, &p, &s);
    assert_eq!(invoke::<Address>(&e, &target, "get_admin", vec![&e]), old);
    auth(&e, &fresh, "accept_management", vec![&e]);
    next.accept_management();
    e.authorize(&[]);
    assert!(next.is_adopted());
    assert_eq!(invoke::<Address>(&e, &target, "get_admin", vec![&e]), fresh);
    auth(&e, &old, "pause_now", vec![&e]);
    assert!(previous.try_pause_now().is_err());
}

#[test]
fn constructor_rejects_invalid_target_governance_and_delay() {
    for scenario in 0..4 {
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let e = env();
            let target = e.register(HELLO, (gov(&e),));
            let delay = match scenario {
                0 => 0,
                1 => e.storage().max_ttl() / 2 + 1,
                _ => 10,
            };
            let governance = if scenario == 2 {
                <Address as soroban_sdk::testutils::Address>::generate(&e)
            } else {
                gov(&e)
            };
            let target = if scenario == 3 { gov(&e) } else { target };
            e.register(HELPER, (target, governance, delay));
        }))
        .is_err());
    }
}

#[cfg(feature = "deployed-v2-test")]
#[test]
fn deployed_v2_wasm_accepts_helper_and_delays_its_real_admin_methods() {
    let e = env();
    let initial_asset = <Address as soroban_sdk::testutils::Address>::generate(&e);
    let target = e.register(
        include_bytes!("../../../admin/target/mainnet-v2.wasm").as_slice(),
        (gov(&e), initial_asset.clone()),
    );
    let (target, helper) = adopt(&e, target);
    let c = TimelockControllerClient::new(&e, &helper);
    assert!(c.try_pause_now().is_err());
    auth(&e, &helper, "pause_now", vec![&e]);
    c.pause_now();
    e.authorize(&[]);
    assert!(invoke::<bool>(&e, &target, "is_paused", vec![&e]));
    let new_asset = <Address as soroban_sdk::testutils::Address>::generate(&e);
    let action = Action::AssetPolicy(new_asset.clone(), true);
    let p = zero(&e);
    let s = salt(&e, 2);
    let id = schedule(&e, &helper, &action, &p, &s);
    assert!(c.try_execute(&action, &p, &s).is_err());
    assert!(!invoke::<bool>(
        &e,
        &target,
        "is_asset_allowed",
        (new_asset.clone(),).into_val(&e)
    ));
    e.ledger().with_mut(|l| l.sequence_number = 1010);
    c.execute(&action, &p, &s);
    assert!(invoke::<bool>(
        &e,
        &target,
        "is_asset_allowed",
        (new_asset,).into_val(&e)
    ));
    let hash = e
        .deployer()
        .upload_contract_wasm(include_bytes!("../../../admin/target/mainnet-v2.wasm").as_slice());
    let upgrade = Action::Upgrade(hash);
    schedule(&e, &helper, &upgrade, &id, &salt(&e, 3));
    e.ledger().with_mut(|l| l.sequence_number = 1020);
    c.execute(&upgrade, &id, &salt(&e, 3));
    assert_eq!(
        invoke::<u32>(&e, &target, "get_schema_version", vec![&e]),
        2
    );
    assert_eq!(
        invoke::<Address>(&e, &target, "get_admin", vec![&e]),
        helper.clone()
    );
    let resume = Action::Unpause;
    schedule(&e, &helper, &resume, &p, &salt(&e, 4));
    assert!(c.try_execute(&resume, &p, &salt(&e, 4)).is_err());
    e.ledger().with_mut(|l| l.sequence_number = 1030);
    c.execute(&resume, &p, &salt(&e, 4));
    assert!(!invoke::<bool>(&e, &target, "is_paused", vec![&e]));
    assert!(invoke::<bool>(
        &e,
        &target,
        "is_asset_allowed",
        (initial_asset,).into_val(&e)
    ));
}

fn schedule(
    e: &Env,
    helper: &Address,
    action: &Action,
    predecessor: &BytesN<32>,
    s: &BytesN<32>,
) -> BytesN<32> {
    auth(
        e,
        helper,
        "schedule",
        (action.clone(), predecessor.clone(), s.clone()).into_val(e),
    );
    let id = TimelockControllerClient::new(e, helper).schedule(action, predecessor, s);
    e.authorize(&[]);
    id
}

#[test]
fn actual_wasm_upgrade_has_delay_exact_payload_and_replay_protection() {
    let e = env();
    let (target, helper) = pair(&e);
    let c = TimelockControllerClient::new(&e, &helper);
    let hash = e.deployer().upload_contract_wasm(NEXT);
    let action = Action::Upgrade(hash.clone());
    let p = zero(&e);
    let s = salt(&e, 2);
    let id = schedule(&e, &helper, &action, &p, &s);
    assert_eq!(c.state(&id), OperationState::Waiting);
    assert_eq!(c.ready_ledger(&id), 1010);
    assert!(c.try_execute(&action, &p, &s).is_err());
    assert!(c.try_execute(&Action::Upgrade(zero(&e)), &p, &s).is_err());
    assert!(c.try_execute(&action, &p, &salt(&e, 3)).is_err());
    assert!(c.try_pause_now().is_err());
    assert!(e
        .try_invoke_contract::<Val, soroban_sdk::Error>(
            &target,
            &symbol_short!("upgrade"),
            vec![&e, hash.clone().into_val(&e)]
        )
        .is_err());
    auth(&e, &helper, "pause_now", vec![&e]);
    c.pause_now();
    e.authorize(&[]);
    e.ledger().with_mut(|l| l.sequence_number = 1009);
    assert!(c.try_execute(&action, &p, &s).is_err());
    e.ledger().with_mut(|l| l.sequence_number = 1010);
    c.execute(&action, &p, &s);
    assert_eq!(c.state(&id), OperationState::Done);
    assert_eq!(
        invoke::<soroban_sdk::String>(&e, &target, "hello", vec![&e]),
        soroban_sdk::String::from_str(&e, "Hello World")
    );
    assert_eq!(invoke::<u32>(&e, &target, "value", vec![&e]), 42u32);
    assert_eq!(
        invoke::<Address>(&e, &target, "get_admin", vec![&e]),
        helper
    );
    assert!(c.try_execute(&action, &p, &s).is_err());
}

#[test]
fn schedule_cancel_and_pause_require_authority_and_cancel_restarts_delay() {
    let e = env();
    let (_, helper) = pair(&e);
    let c = TimelockControllerClient::new(&e, &helper);
    let action = Action::Unpause;
    let p = zero(&e);
    let s = salt(&e, 2);
    assert!(c.try_schedule(&action, &p, &s).is_err());
    let wrong = Address::from_str(
        &e,
        "GD3UEYYZRU53VBAVGEKR6HYQ3USQ3FEBT5BLOYEX356EFOM5SR5774GW",
    );
    e.authorize(&[MockAuth {
        address: &wrong,
        invoke: &MockAuthInvoke {
            contract: &helper,
            fn_name: "schedule",
            args: (action.clone(), p.clone(), s.clone()).into_val(&e),
            sub_invokes: &[],
        },
    }]);
    assert!(c.try_schedule(&action, &p, &s).is_err());
    let id = schedule(&e, &helper, &action, &p, &s);
    assert!(c.try_cancel(&id).is_err());
    auth(&e, &helper, "cancel", (id.clone(),).into_val(&e));
    c.cancel(&id);
    e.authorize(&[]);
    assert_eq!(c.state(&id), OperationState::Unset);
    e.ledger().with_mut(|l| l.sequence_number = 1010);
    assert!(c.try_execute(&action, &p, &s).is_err());
    schedule(&e, &helper, &action, &p, &s);
    assert_eq!(c.ready_ledger(&id), 1020);
    assert!(c.try_execute(&action, &p, &s).is_err());
}

#[test]
fn failed_target_execution_rolls_back_done_and_predecessor_is_enforced() {
    let e = env();
    let (_, helper) = pair(&e);
    let c = TimelockControllerClient::new(&e, &helper);
    let action = Action::Upgrade(e.deployer().upload_contract_wasm(NEXT));
    let p = zero(&e);
    let s = salt(&e, 2);
    let id = schedule(&e, &helper, &action, &p, &s);
    let follow = schedule(&e, &helper, &Action::Unpause, &id, &salt(&e, 3));
    e.ledger().with_mut(|l| l.sequence_number = 1010);
    assert!(c.try_execute(&Action::Unpause, &id, &salt(&e, 3)).is_err());
    assert!(c.try_execute(&action, &p, &s).is_err()); // not paused
    assert_eq!(c.state(&id), OperationState::Ready);
    auth(&e, &helper, "pause_now", vec![&e]);
    c.pause_now();
    e.authorize(&[]);
    c.execute(&action, &p, &s);
    c.execute(&Action::Unpause, &id, &salt(&e, 3));
    assert_eq!(c.state(&follow), OperationState::Done);
}

#[contract]
pub struct RejectAcceptance;
#[contractimpl]
impl RejectAcceptance {
    pub fn __constructor(e: Env, authority: Address) {
        e.storage()
            .instance()
            .set(&symbol_short!("admin"), &authority);
    }
    pub fn get_admin(e: Env) -> Address {
        e.storage().instance().get(&symbol_short!("admin")).unwrap()
    }
    pub fn get_pending_admin(e: Env) -> Option<Address> {
        e.storage().instance().get(&symbol_short!("pending"))
    }
    pub fn propose_admin(e: Env, new_admin: Address) {
        Self::get_admin(e.clone()).require_auth();
        e.storage()
            .instance()
            .set(&symbol_short!("pending"), &new_admin);
    }
    pub fn accept_admin(_e: Env) {
        panic!("refuse acceptance after nomination");
    }
}

#[test]
fn nominated_controller_requires_governance_auth_to_accept() {
    let e = env();
    let authority = gov(&e);
    let target = e.register(HELLO, (authority.clone(),));
    let helper = e.register(HELPER, (target.clone(), authority.clone(), 10u32));
    auth(&e, &target, "propose_admin", (helper.clone(),).into_val(&e));
    invoke::<()>(&e, &target, "propose_admin", (helper.clone(),).into_val(&e));
    e.authorize(&[]);
    assert!(TimelockControllerClient::new(&e, &helper)
        .try_accept_management()
        .is_err());
    assert_eq!(
        invoke::<Address>(&e, &target, "get_admin", vec![&e]),
        authority
    );
    assert!(!TimelockControllerClient::new(&e, &helper).is_adopted());
}

#[test]
fn pending_governance_handoff_blocks_management_acceptance() {
    let e = env();
    let authority = gov(&e);
    let target = e.register(HELLO, (authority.clone(),));
    let helper = e.register(HELPER, (target.clone(), authority, 10u32));
    let client = TimelockControllerClient::new(&e, &helper);
    let successor = Address::from_str(
        &e,
        "GD3UEYYZRU53VBAVGEKR6HYQ3USQ3FEBT5BLOYEX356EFOM5SR5774GW",
    );
    auth(
        &e,
        &helper,
        "propose_governance",
        (successor.clone(),).into_val(&e),
    );
    client.propose_governance(&successor);
    auth(&e, &target, "propose_admin", (helper.clone(),).into_val(&e));
    invoke::<()>(&e, &target, "propose_admin", (helper.clone(),).into_val(&e));
    auth(&e, &helper, "accept_management", vec![&e]);
    assert_eq!(
        client.try_accept_management(),
        Err(Ok(soroban_sdk::Error::from_contract_error(
            crate::Error::InvalidHandoff as u32
        )))
    );
    assert!(!client.is_adopted());
}

#[test]
fn failed_acceptance_keeps_the_existing_admin_and_pending_nomination() {
    let e = env();
    let authority = gov(&e);
    let target = e.register(RejectAcceptance, (authority.clone(),));
    let helper = e.register(HELPER, (target.clone(), authority.clone(), 10u32));
    auth(&e, &target, "propose_admin", (helper.clone(),).into_val(&e));
    invoke::<()>(&e, &target, "propose_admin", (helper.clone(),).into_val(&e));
    auth(&e, &helper, "accept_management", vec![&e]);
    let client = TimelockControllerClient::new(&e, &helper);
    assert!(client.try_accept_management().is_err());
    assert!(!client.is_adopted());
    assert_eq!(
        invoke::<Address>(&e, &target, "get_admin", vec![&e]),
        authority
    );
    assert_eq!(
        invoke::<Option<Address>>(&e, &target, "get_pending_admin", vec![&e]),
        Some(helper)
    );
}
