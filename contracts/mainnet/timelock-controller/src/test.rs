extern crate std;

use soroban_sdk::{
    auth::{Context, ContractContext},
    contract, contractimpl, symbol_short,
    testutils::{Address as _, BytesN as _, Ledger, MockAuth, MockAuthInvoke},
    vec, Address, BytesN, Env, IntoVal, Symbol, Vec,
};
use stellar_governance::timelock::{OperationState, TimelockError};

use crate::{OperationMeta, TimelockController, TimelockControllerClient};

// Helper function to create empty BytesN<32>
fn empty(e: &Env) -> BytesN<32> {
    BytesN::<32>::from_array(e, &[0u8; 32])
}

// Mock target contract for testing
#[contract]
pub struct TargetContract;

#[contractimpl]
impl TargetContract {
    pub fn set_value(e: &Env, value: u32) -> u32 {
        e.storage().instance().set(&symbol_short!("value"), &value);
        value
    }

    pub fn get_value(e: &Env) -> u32 {
        e.storage()
            .instance()
            .get(&symbol_short!("value"))
            .unwrap_or(0)
    }
}

#[test]
fn initialization() {
    let e = Env::default();
    e.mock_all_auths();

    let proposer = Address::generate(&e);
    let executor = Address::generate(&e);
    let admin = Address::generate(&e);

    let timelock = e.register(
        TimelockController,
        (
            10u32,
            vec![&e, proposer.clone()],
            vec![&e, executor.clone()],
            Some(admin.clone()),
        ),
    );

    let client = TimelockControllerClient::new(&e, &timelock);

    assert_eq!(client.get_min_delay(), 10);

    // Check roles are granted
    assert!(client
        .has_role(&proposer, &symbol_short!("proposer"))
        .is_some());
    assert!(client
        .has_role(&proposer, &symbol_short!("canceller"))
        .is_some());
    assert!(client
        .has_role(&executor, &symbol_short!("executor"))
        .is_some());
    assert_eq!(client.get_admin(), Some(admin));
}

#[test]
fn inherited_access_control_management_and_read_surface_are_consistent() {
    let e = Env::default();
    e.mock_all_auths();

    let proposer = Address::generate(&e);
    let executor = Address::generate(&e);
    let admin = Address::generate(&e);
    let delegated_admin = Address::generate(&e);
    let candidate = Address::generate(&e);
    let timelock = e.register(
        TimelockController,
        (
            10u32,
            vec![&e, proposer.clone()],
            vec![&e, executor.clone()],
            Some(admin.clone()),
        ),
    );
    let client = TimelockControllerClient::new(&e, &timelock);
    let proposer_role = symbol_short!("proposer");
    let executor_role = symbol_short!("executor");
    let canceller_role = symbol_short!("canceller");
    let delegated_role = symbol_short!("execadm");

    assert_eq!(client.get_role_member_count(&proposer_role), 1);
    assert_eq!(client.get_role_member(&proposer_role, &0), proposer);
    assert_eq!(client.get_role_member_count(&executor_role), 1);
    assert_eq!(client.get_role_member(&executor_role, &0), executor);
    assert_eq!(client.get_role_member_count(&canceller_role), 1);
    assert_eq!(client.get_role_admin(&executor_role), None);
    assert!(client.try_get_role_member(&executor_role, &1).is_err());
    let roles = client.get_existing_roles();
    for role in [proposer_role, executor_role.clone(), canceller_role.clone()] {
        assert!(roles.iter().any(|existing| existing == role));
    }

    client.grant_role(&delegated_admin, &delegated_role, &admin);
    client.set_role_admin(&executor_role, &delegated_role);
    assert_eq!(client.get_role_admin(&executor_role), Some(delegated_role));
    client.grant_role(&candidate, &executor_role, &delegated_admin);
    assert!(client.has_role(&candidate, &executor_role).is_some());
    client.revoke_role(&candidate, &executor_role, &delegated_admin);
    assert_eq!(client.has_role(&candidate, &executor_role), None);

    client.grant_role(&candidate, &canceller_role, &admin);
    client.renounce_role(&canceller_role, &candidate);
    assert_eq!(client.has_role(&candidate, &canceller_role), None);

    client.update_delay(&20, &admin);
    assert_eq!(client.get_min_delay(), 20);
}

#[test]
fn inherited_access_control_admin_transfer_and_negative_paths_are_enforced() {
    let e = Env::default();
    e.mock_all_auths();

    let proposer = Address::generate(&e);
    let admin = Address::generate(&e);
    let next_admin = Address::generate(&e);
    let attacker = Address::generate(&e);
    let candidate = Address::generate(&e);
    let timelock = e.register(
        TimelockController,
        (
            10u32,
            vec![&e, proposer],
            Vec::<Address>::new(&e),
            Some(admin.clone()),
        ),
    );
    let client = TimelockControllerClient::new(&e, &timelock);
    let executor_role = symbol_short!("executor");

    assert!(client
        .try_grant_role(&candidate, &executor_role, &attacker)
        .is_err());
    assert!(client
        .try_revoke_role(&candidate, &executor_role, &attacker)
        .is_err());
    assert!(client.try_renounce_role(&executor_role, &attacker).is_err());

    client.transfer_admin_role(&next_admin, &100);
    assert_eq!(client.get_admin(), Some(admin.clone()));
    assert!(client.try_renounce_admin().is_err());
    client.accept_admin_transfer();
    assert_eq!(client.get_admin(), Some(next_admin.clone()));
    assert!(client
        .try_grant_role(&candidate, &executor_role, &admin)
        .is_err());
    client.grant_role(&candidate, &executor_role, &next_admin);
    assert!(client.has_role(&candidate, &executor_role).is_some());

    e.set_auths(&[]);
    assert!(client
        .try_revoke_role(&candidate, &executor_role, &next_admin)
        .is_err());
    assert!(client.try_transfer_admin_role(&attacker, &100).is_err());
    assert!(client.try_renounce_admin().is_err());
}

#[test]
fn schedule_and_execute_operation() {
    let e = Env::default();
    e.mock_all_auths();

    let proposer = Address::generate(&e);
    let executor = Address::generate(&e);
    let target = e.register(TargetContract, ());

    let timelock = e.register(
        TimelockController,
        (
            10u32,
            vec![&e, proposer.clone()],
            vec![&e, executor.clone()],
            None::<Address>,
        ),
    );

    let client = TimelockControllerClient::new(&e, &timelock);
    let target_client = TargetContractClient::new(&e, &target);

    let args = vec![&e, 42u32.into_val(&e)];
    let scheduled_at = e.ledger().sequence();
    let operation_id = client.schedule(
        &target,
        &symbol_short!("set_value"),
        &args,
        &empty(&e),
        &empty(&e),
        &10,
        &proposer,
    );

    assert_eq!(
        operation_id,
        client.hash_operation(
            &target,
            &symbol_short!("set_value"),
            &args,
            &empty(&e),
            &empty(&e),
        )
    );
    assert_eq!(
        client.get_operation_ledger(&operation_id),
        scheduled_at + 10
    );

    assert!(client.get_operation_state(&operation_id) != OperationState::Unset);
    assert!(matches!(
        client.get_operation_state(&operation_id),
        OperationState::Waiting | OperationState::Ready
    ));
    assert_ne!(
        client.get_operation_state(&operation_id),
        OperationState::Ready
    );

    // Advance ledgers to make operation ready
    e.ledger().with_mut(|li| li.sequence_number += 10);

    assert_eq!(
        client.get_operation_state(&operation_id),
        OperationState::Ready
    );

    client.execute(
        &target,
        &symbol_short!("set_value"),
        &args,
        &empty(&e),
        &empty(&e),
        &Some(executor),
    );

    assert_eq!(target_client.get_value(), 42);
    assert_eq!(
        client.get_operation_state(&operation_id),
        OperationState::Done
    );
}

#[test]
fn schedule_and_execute_operation_no_executors() {
    let e = Env::default();
    e.mock_all_auths();

    let proposer = Address::generate(&e);
    let target = e.register(TargetContract, ());

    let timelock = e.register(
        TimelockController,
        // no executors
        (
            10u32,
            vec![&e, proposer.clone()],
            Vec::<Address>::new(&e),
            None::<Address>,
        ),
    );

    let client = TimelockControllerClient::new(&e, &timelock);
    let target_client = TargetContractClient::new(&e, &target);

    let args = vec![&e, 42u32.into_val(&e)];
    let operation_id = client.schedule(
        &target,
        &symbol_short!("set_value"),
        &args,
        &empty(&e),
        &empty(&e),
        &10,
        &proposer,
    );

    assert!(client.get_operation_state(&operation_id) != OperationState::Unset);
    assert!(matches!(
        client.get_operation_state(&operation_id),
        OperationState::Waiting | OperationState::Ready
    ));
    assert_ne!(
        client.get_operation_state(&operation_id),
        OperationState::Ready
    );

    e.ledger().with_mut(|li| li.sequence_number += 10);

    assert_eq!(
        client.get_operation_state(&operation_id),
        OperationState::Ready
    );

    client.execute(
        &target,
        &symbol_short!("set_value"),
        &args,
        &empty(&e),
        &empty(&e),
        // any address
        &None,
    );

    assert_eq!(target_client.get_value(), 42);
    assert_eq!(
        client.get_operation_state(&operation_id),
        OperationState::Done
    );
}

#[test]
fn schedule_and_execute_self_admin_operation() {
    let e = Env::default();

    let proposer = Address::generate(&e);
    let executor = Address::generate(&e);

    let timelock = e.register(
        TimelockController,
        (
            10u32,
            vec![&e, proposer.clone()],
            vec![&e, executor.clone()],
            None::<Address>,
        ),
    );

    let client = TimelockControllerClient::new(&e, &timelock);

    let args = vec![&e, 42u32.into_val(&e)];
    let operation_id = client
        .mock_auths(&[MockAuth {
            address: &proposer,
            invoke: &MockAuthInvoke {
                contract: &timelock,
                fn_name: "schedule",
                args: (
                    timelock.clone(),
                    Symbol::new(&e, "update_delay"),
                    args.clone(),
                    empty(&e),
                    empty(&e),
                    10u32,
                    proposer.clone(),
                )
                    .into_val(&e),
                sub_invokes: &[],
            },
        }])
        .schedule(
            &timelock,
            &Symbol::new(&e, "update_delay"),
            &args,
            &empty(&e),
            &empty(&e),
            &10,
            &proposer,
        );

    // Check operation is pending
    assert!(client.get_operation_state(&operation_id) != OperationState::Unset);
    assert!(matches!(
        client.get_operation_state(&operation_id),
        OperationState::Waiting | OperationState::Ready
    ));
    assert_ne!(
        client.get_operation_state(&operation_id),
        OperationState::Ready
    );

    e.ledger().with_mut(|li| li.sequence_number += 10);

    assert_eq!(
        client.get_operation_state(&operation_id),
        OperationState::Ready
    );

    // Mock executor's require_auth_for_args() that's called in `__check_auth`
    e.mock_auths(&[MockAuth {
        address: &executor,
        invoke: &MockAuthInvoke {
            contract: &timelock,
            fn_name: "__check_auth",
            args: (
                Symbol::new(&e, "execute_op"),
                timelock.clone(),
                Symbol::new(&e, "update_delay"),
                args.clone(),
                empty(&e),
                empty(&e),
            )
                .into_val(&e),
            sub_invokes: &[],
        },
    }]);

    // `__check_auth` can't be called directly, hence we need to use
    // `try_invoke_contract_check_auth` testing utility that emulates being
    // called by the Soroban host during a `require_auth` call.
    e.try_invoke_contract_check_auth::<TimelockError>(
        &timelock,
        &BytesN::random(&e),
        vec![
            &e,
            OperationMeta {
                predecessor: empty(&e),
                salt: empty(&e),
                executor: Some(executor.clone()),
            },
        ]
        .into_val(&e),
        &vec![
            &e,
            Context::Contract(ContractContext {
                contract: timelock.clone(),
                fn_name: Symbol::new(&e, "update_delay"),
                args: args.clone(),
            }),
        ],
    )
    .unwrap();

    assert_eq!(
        client.get_operation_state(&operation_id),
        OperationState::Done
    );
}

#[test]
fn self_admin_authorization_rejects_malformed_external_and_unscheduled_contexts() {
    let e = Env::default();
    let proposer = Address::generate(&e);
    let external = e.register(TargetContract, ());
    let timelock = e.register(
        TimelockController,
        (
            10u32,
            vec![&e, proposer],
            Vec::<Address>::new(&e),
            None::<Address>,
        ),
    );
    let payload = BytesN::random(&e);
    let args = vec![&e, 42u32.into_val(&e)];
    let meta = vec![
        &e,
        OperationMeta {
            predecessor: empty(&e),
            salt: empty(&e),
            executor: None,
        },
    ];

    let external_context = vec![
        &e,
        Context::Contract(ContractContext {
            contract: external,
            fn_name: Symbol::new(&e, "set_value"),
            args: args.clone(),
        }),
    ];
    assert!(e
        .try_invoke_contract_check_auth::<TimelockError>(
            &timelock,
            &payload,
            Vec::<OperationMeta>::new(&e).into_val(&e),
            &external_context,
        )
        .is_err());
    assert!(e
        .try_invoke_contract_check_auth::<TimelockError>(
            &timelock,
            &payload,
            meta.clone().into_val(&e),
            &external_context,
        )
        .is_err());

    let unscheduled_self_context = vec![
        &e,
        Context::Contract(ContractContext {
            contract: timelock.clone(),
            fn_name: Symbol::new(&e, "update_delay"),
            args,
        }),
    ];
    assert!(e
        .try_invoke_contract_check_auth::<TimelockError>(
            &timelock,
            &payload,
            meta.into_val(&e),
            &unscheduled_self_context,
        )
        .is_err());
}

#[test]
fn cancel_operation() {
    let e = Env::default();
    e.mock_all_auths();

    let proposer = Address::generate(&e);
    let target = e.register(TargetContract, ());

    let timelock = e.register(
        TimelockController,
        (
            10u32,
            vec![&e, proposer.clone()],
            Vec::<Address>::new(&e),
            None::<Address>,
        ),
    );

    let client = TimelockControllerClient::new(&e, &timelock);

    let args = vec![&e, 42u32.into_val(&e)];
    let operation_id = client.schedule(
        &target,
        &symbol_short!("set_value"),
        &args,
        &empty(&e),
        &empty(&e),
        &10,
        &proposer,
    );

    assert!(matches!(
        client.get_operation_state(&operation_id),
        OperationState::Waiting | OperationState::Ready
    ));

    client.cancel(&operation_id, &proposer);

    // Check operation is no longer existing
    assert_eq!(
        client.get_operation_state(&operation_id),
        OperationState::Unset
    );
}

#[test]
#[should_panic(expected = "#4001")]
fn schedule_with_insufficient_delay() {
    let e = Env::default();
    e.mock_all_auths();

    let proposer = Address::generate(&e);
    let target = e.register(TargetContract, ());

    let timelock = e.register(
        TimelockController,
        (
            10u32,
            vec![&e, proposer.clone()],
            Vec::<Address>::new(&e),
            None::<Address>,
        ),
    );

    let client = TimelockControllerClient::new(&e, &timelock);

    // Try to schedule with delay less than minimum
    let args = vec![&e, 42u32.into_val(&e)];
    client.schedule(
        &target,
        &symbol_short!("set_value"),
        &args,
        &empty(&e),
        &empty(&e),
        &5, // Less than min delay of 10
        &proposer,
    );
}

#[test]
#[should_panic(expected = "#4002")]
fn execute_before_ready() {
    let e = Env::default();
    e.mock_all_auths();

    let proposer = Address::generate(&e);
    let executor = Address::generate(&e);
    let target = e.register(TargetContract, ());

    let timelock = e.register(
        TimelockController,
        (
            10u32,
            vec![&e, proposer.clone()],
            vec![&e, executor.clone()],
            None::<Address>,
        ),
    );

    let client = TimelockControllerClient::new(&e, &timelock);

    // Schedule operation
    let args = vec![&e, 42u32.into_val(&e)];
    client.schedule(
        &target,
        &symbol_short!("set_value"),
        &args,
        &empty(&e),
        &empty(&e),
        &10,
        &proposer,
    );

    // Try to execute before delay passes (should panic)
    client.execute(
        &target,
        &symbol_short!("set_value"),
        &args,
        &empty(&e),
        &empty(&e),
        &Some(executor),
    );
}

#[test]
fn every_timelock_mutator_rejects_wrong_roles_and_missing_authorization() {
    let e = Env::default();
    e.mock_all_auths();

    let proposer = Address::generate(&e);
    let executor = Address::generate(&e);
    let admin = Address::generate(&e);
    let attacker = Address::generate(&e);
    let target = e.register(TargetContract, ());
    let timelock = e.register(
        TimelockController,
        (
            10u32,
            vec![&e, proposer.clone()],
            vec![&e, executor.clone()],
            Some(admin.clone()),
        ),
    );
    let client = TimelockControllerClient::new(&e, &timelock);
    let args = vec![&e, 42u32.into_val(&e)];
    let predecessor = empty(&e);
    let salt = BytesN::from_array(&e, &[3; 32]);

    assert!(client
        .try_schedule(
            &target,
            &symbol_short!("set_value"),
            &args,
            &predecessor,
            &salt,
            &10,
            &attacker,
        )
        .is_err());
    let operation_id = client.schedule(
        &target,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt,
        &10,
        &proposer,
    );
    assert!(client.try_cancel(&operation_id, &attacker).is_err());
    e.ledger().with_mut(|li| li.sequence_number += 10);
    assert!(client
        .try_execute(
            &target,
            &symbol_short!("set_value"),
            &args,
            &predecessor,
            &salt,
            &Some(attacker),
        )
        .is_err());

    e.set_auths(&[]);
    let second_salt = BytesN::from_array(&e, &[4; 32]);
    assert!(client
        .try_schedule(
            &target,
            &symbol_short!("set_value"),
            &args,
            &predecessor,
            &second_salt,
            &10,
            &proposer,
        )
        .is_err());
    assert!(client.try_cancel(&operation_id, &proposer).is_err());
    assert!(client
        .try_execute(
            &target,
            &symbol_short!("set_value"),
            &args,
            &predecessor,
            &salt,
            &Some(executor),
        )
        .is_err());
    assert!(client.try_update_delay(&20, &admin).is_err());
}
