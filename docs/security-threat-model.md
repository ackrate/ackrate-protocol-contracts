# Mainnet security threat model

Status: release gate
Last reviewed: 2026-08-27

This document is a prerequisite for a production release. It is not a closing
artifact. A change to the contract interface, persistent schema, authority
model, asset policy, payment path, or x402 adapter must update this model and
pass the continuous security suite before release.

## Security objective

Ackrate must never rely on an SDK, model, merchant, UI, cached mandate, or x402
message to decide whether value moves. The deployed `MandateRegistry` is the
enforcement boundary. Its `execute_payment` entry point re-reads durable state,
authenticates the bound agent, validates the merchant, asset, expiry, sequence,
status, and remaining amount, consumes the mandate, and invokes SEP-41
`transfer_from` in one atomic Soroban transaction.

The x402 exchange is deliberately outside that invariant. It is an adapter that
may change as x402 evolves from v0.1; the on-chain mandate and payment rules do
not depend on the HTTP request or response shape.

## Assets and trust boundaries

| Asset | Owner | Required protection |
|---|---|---|
| User USDC allowance | User | Only `MandateRegistry` is approved as spender; the agent and app receive no allowance. |
| Mandate state | User / contract | Registration requires user authorization; consumption is contract-owned and atomic. |
| Sequence and spent amount | Contract | Never accepted from an SDK cache; read and updated on-chain for every payment. |
| Merchant and asset scope | User / governance | Immutable per mandate; assets must also pass the governed allowlist. |
| Upgrade authority | 2-of-3 authority through TimelockController | Exact operation binding, minimum delay, and contract-held privileged roles. |
| Emergency stop | Separate pauser | May stop value movement, but cannot unpause, change policy, or upgrade. |
| Release artifact | Reviewers | Exact source, toolchain, WASM hash, provenance, constructor arguments, and on-chain hash must agree. |

Trusted dependencies are limited to Stellar consensus and the Soroban host,
the canonical Circle USDC Stellar Asset Contract recorded in the deployment
manifest, and the explicitly published governance accounts. The SDK, CLI,
wallet UI, x402 transport, consumer agent, fulfillment agent, merchant server,
RPC provider, database, and model output are untrusted inputs.

## Threats, controls, and continuous evidence

| Threat | Contract control | Continuous test evidence |
|---|---|---|
| Unauthorized registration, payment, revocation, pause, policy change, or upgrade | `require_auth` plus OpenZeppelin role checks | `user_agent_and_revocation_authorizations_are_host_enforced`; `governance_functions_require_both_role_and_authorization`; `every_timelock_mutator_rejects_wrong_roles_and_missing_authorization` |
| Expired mandate | Ledger timestamp is checked during registration and every payment | `duplicate_unknown_overspend_expiry_revocation_scope_and_replay_fail`; `hostile_extension_cannot_bypass_revocation_expiry_or_pause` |
| Overspend or zero/negative amount | Remaining budget is recomputed from durable state | `duplicate_unknown_overspend_expiry_revocation_scope_and_replay_fail`; `hostile_extension_is_still_bounded_by_budget_and_sequence` |
| Replay or out-of-order consumption | Caller supplies the current sequence; the contract compares and increments it atomically | `duplicate_unknown_overspend_expiry_revocation_scope_and_replay_fail`; `hostile_extension_is_still_bounded_by_budget_and_sequence` |
| Merchant or asset substitution | Merchant is fixed in the mandate; asset is fixed and allowlisted | `asset_allowlist_and_lifetime_are_enforced_at_registration`; `hostile_extension_cannot_select_a_different_merchant_or_asset` |
| Cached or skipped SDK validation | `validate_mandate` is only a dry run; `execute_payment` always performs authoritative validation again | `happy_path_moves_exactly_the_consumed_amount`; all payment negative cases |
| Reentrancy through a malicious token | State is consumed before transfer and Soroban rejects recursive entry | `reentrancy_via_evil_token` |
| Token allowance or transfer failure | Soroban transaction rollback restores mandate state and balances | `allowance_failure_reverts_consumption_and_transfer` |
| Alternate money path introduced by an extension | Registry exposes no plugin hook, callback registry, or extension allowance | `hostile_extension_has_no_token_allowance_or_second_money_path`; `direct_caller_cannot_bypass_a_contract_agent` |
| Emergency key expands its authority | Separate pauser and unpauser roles; pauser can only stop | `emergency_pause_is_one_key_but_unpause_is_separate` |
| Unauthorized or early upgrade | Registry upgrader is the timelock; operation hash binds target, function, args, predecessor, and salt; minimum delay enforced | `governance_functions_require_both_role_and_authorization`; `canonical_timelock_binds_and_executes_the_exact_policy_change`; `schedule_with_insufficient_delay`; `execute_before_ready`; `every_timelock_mutator_rejects_wrong_roles_and_missing_authorization` |
| Artifact substitution | Pinned toolchain, exact hashes, GitHub provenance, and observed on-chain hashes | `gatecheck-mainnet.sh`; `deployment-manifest.json`; release workflow |
| Vulnerable or yanked dependency | The lockfile dependency gate is a required CI job; actionable findings fail the build | `scripts/security-scan.sh`; `docs/security-scan-report.md` |

The executable sources are under
`contracts/mainnet/mandate-registry/src/` and
`contracts/mainnet/timelock-controller/src/`. The test names above are stable
reviewer entry points and run on every push and pull request.

## Contract surface review

### MandateRegistry

- Construction and public reads (`__constructor`, `is_paused`,
  `is_asset_allowed`, `get_schema_version`, `keep_alive`, `role_ids`,
  `get_mandate`) are covered by initialization, unknown-state, TTL, and role
  assertions.
- User lifecycle (`register_mandate`, `revoke_mandate`) is covered by positive
  state transitions and negative authorization, lifetime, duplicate, asset,
  and revoked-state cases.
- Payment (`validate_mandate`, `execute_payment`) is covered by positive
  settlement and negative amount, caller, merchant, expiry, budget, sequence,
  pause, reentrancy, and token-failure cases.
- Governance (`pause`, `unpause`, `set_asset_allowed`, `upgrade` and inherited
  access-control methods) is covered by wrong-role, missing-authorization,
  separated-authority, exact-operation, and delay cases.

### TimelockController

- Construction and read helpers are covered by initialization and operation
  state transitions.
- `schedule`, `execute`, `cancel`, and `update_delay` are each exercised on a
  valid path and rejected for a wrong role or missing authorization.
- Self-administration is bound to the exact target, function, arguments,
  predecessor, salt, executor policy, and ready operation.

## Governance custody and recovery

The final authority uses three independently held signer keys (Future, Max,
and Alex) with weight 1 and low/medium/high thresholds of 2. Public signer
addresses are recorded in the reviewed authority manifest; recovery phrases
and secret keys are never stored in this repository.

- **Normal action:** one signer prepares the exact transaction; a second signer
  independently verifies network, contract, function, arguments, and hash
  before adding a signature. The transaction is submitted only after 2-of-3.
- **Rotation:** schedule the exact signer/threshold change through the current
  authority and timelock where applicable; wait the full delay; obtain two
  current signatures; execute; then update and re-run the public authority
  manifest preflight.
- **One key lost:** the remaining two signers rotate the lost signer without
  lowering any threshold. The lost signer is never reused.
- **Two keys lost:** no unilateral recovery path exists. Payments may be paused
  with the separate emergency key, but governance cannot be bypassed. Recovery
  requires the documented off-chain custody process and a reviewed protocol
  migration; no hidden administrator or bootstrap key is retained.
- **Compromise:** pause immediately, publish the incident identifier, prepare a
  timelocked rotation or upgrade with the unaffected quorum, and resume only
  after independent state and artifact verification.

Friday's final activation must verify the three live signers and thresholds
against `scripts/preflight-mainnet.mjs` before any transaction is submitted.

## Residual risk and release rules

- x402 v0.1 will change. Adapters may translate new wire formats, but they may
  not add another settlement path or weaken the contract call.
- A fully immutable final deployment makes an incomplete threat model
  permanent. Immutability therefore requires a reviewed model, diagrams,
  negative suite, dependency scan, exact-byte artifact gate, and live drills.
- RPC and merchant outages can delay service but must not weaken authorization.
  Clients retry safely; settlement evidence remains bound to one transaction
  and one fulfillment claim.
- The final release is stopped by a failing security test, unreviewed contract
  surface change, actionable dependency finding, artifact mismatch, authority
  mismatch, or missing live payment/rejection receipt.
