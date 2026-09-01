# Legacy Mainnet canary exported-function coverage

> Historical record for the earlier Registry + TimelockController canary. The
> deployed Mainnet V2 security result is
> [`mainnet-v2-security-verification.md`](mainnet-v2-security-verification.md).

Status: release gate
Last reviewed: 2026-08-27

This matrix maps every exported mainnet contract function to executable test
evidence. Read-only functions have state and boundary checks; functions that
can change state or authority have both accepted and rejected paths.

## MandateRegistry

| Exported function | Accepted-path evidence | Rejected-path or boundary evidence |
|---|---|---|
| `__constructor` | `constructor_sets_openzeppelin_roles_schema_and_asset` | One-time constructor behavior is host-enforced; configured roles, schema, asset, and pause state are asserted after creation. |
| `pause`, `unpause`, `is_paused` | `emergency_pause_is_one_key_but_unpause_is_separate` | `governance_functions_require_both_role_and_authorization`; `unpause_requires_the_unpauser_signature` |
| `set_asset_allowed`, `is_asset_allowed` | `canonical_timelock_binds_and_executes_the_exact_policy_change` | `governance_functions_require_both_role_and_authorization`; `asset_allowlist_and_lifetime_are_enforced_at_registration` |
| `get_schema_version` | `constructor_sets_openzeppelin_roles_schema_and_asset`; `canonical_timelock_upgrades_registry_at_same_address_and_preserves_state` | Read-only; schema preservation is asserted after replacement. |
| `keep_alive` | `keep_alive_extends_contract_instance_ttl` | Permissionless by design; the test proves only TTL changes. |
| `role_ids` | `inherited_access_control_grant_revoke_and_read_surface_are_consistent` | Read-only; exact role identifiers are asserted. |
| `register_mandate` | `happy_path_moves_exactly_the_consumed_amount` | `asset_allowlist_and_lifetime_are_enforced_at_registration`; `duplicate_unknown_overspend_expiry_revocation_scope_and_replay_fail`; `user_agent_and_revocation_authorizations_are_host_enforced` |
| `validate_mandate` | `happy_path_moves_exactly_the_consumed_amount` | `duplicate_unknown_overspend_expiry_revocation_scope_and_replay_fail` covers zero, negative, expired, revoked, exhausted, budget, and merchant-scope rejection. |
| `execute_payment` | `happy_path_moves_exactly_the_consumed_amount` | Replay, expiry, revocation, scope, budget, pause, missing agent authorization, allowance rollback, reentrancy, and hostile-extension cases are covered across the Registry, reentry, and hostile-extension suites. |
| `revoke_mandate` | `duplicate_unknown_overspend_expiry_revocation_scope_and_replay_fail` | `user_agent_and_revocation_authorizations_are_host_enforced` |
| `get_mandate` | `happy_path_moves_exactly_the_consumed_amount` | Unknown ID rejection is covered by `duplicate_unknown_overspend_expiry_revocation_scope_and_replay_fail`. |
| `upgrade` | `canonical_timelock_upgrades_registry_at_same_address_and_preserves_state` | `governance_functions_require_both_role_and_authorization`; early execution is rejected before the exact ready operation succeeds. |

The inherited access-control exports `get_admin`, `get_role_member_count`,
`get_role_member`, `get_existing_roles`, `has_role`, `get_role_admin`,
`grant_role`, `revoke_role`, `renounce_role`, `set_role_admin`,
`transfer_admin_role`, `accept_admin_transfer`, and `renounce_admin` are covered
by these four tests:

- `inherited_access_control_mutators_reject_wrong_authority_and_missing_auth`
- `inherited_access_control_grant_revoke_and_read_surface_are_consistent`
- `inherited_access_control_role_admin_delegation_and_renunciation_work`
- `inherited_access_control_admin_transfer_and_renunciation_are_enforced`

`expired_admin_transfer_cannot_be_accepted` separately proves that an expired
administrator transfer cannot be claimed.

## TimelockController

| Exported function | Accepted-path evidence | Rejected-path or boundary evidence |
|---|---|---|
| `__constructor` | `initialization` | Post-creation delay, administrator, proposer, executor, and automatic canceller roles are asserted. |
| `get_min_delay` | `initialization`; `inherited_access_control_management_and_read_surface_are_consistent` | Read-only; the updated value is asserted after an authorized change. |
| `hash_operation`, `get_operation_ledger`, `get_operation_state` | `schedule_and_execute_operation` | The computed ID, exact ready ledger, and Waiting/Ready/Done transitions are asserted. |
| `schedule` | `schedule_and_execute_operation`; `schedule_and_execute_operation_no_executors` | `schedule_with_insufficient_delay`; `every_timelock_mutator_rejects_wrong_roles_and_missing_authorization` |
| `execute` | `schedule_and_execute_operation`; `schedule_and_execute_operation_no_executors` | `execute_before_ready`; `every_timelock_mutator_rejects_wrong_roles_and_missing_authorization` |
| `cancel` | `cancel_operation` | `every_timelock_mutator_rejects_wrong_roles_and_missing_authorization` |
| `update_delay` | `inherited_access_control_management_and_read_surface_are_consistent`; scheduled authorization context in `schedule_and_execute_self_admin_operation` | Missing authorization is rejected by `every_timelock_mutator_rejects_wrong_roles_and_missing_authorization`. |
| `__check_auth` | `schedule_and_execute_self_admin_operation` | `self_admin_authorization_rejects_malformed_external_and_unscheduled_contexts` |

The same 13 inherited access-control exports listed for MandateRegistry are
covered on TimelockController by
`inherited_access_control_management_and_read_surface_are_consistent` and
`inherited_access_control_admin_transfer_and_negative_paths_are_enforced`.

## Reproduce

Run the complete repository gate:

```bash
./scripts/gatecheck-contracts.sh
./scripts/security-scan.sh
```

The first command formats, lints, builds, and tests all current contract suites.
The second enforces the pinned dependency policy for both mainnet contracts.
