# Mainnet V2 threat model

**Reviewed:** 2026-09-07 (Bangkok, UTC+7). **Scope:** deployed V2 `0.4.1`,
schema `2`, and its contract-security release gates. This is the current model;
the older `security-threat-model.md` describes a different, historical canary.

The target is [Mainnet MandateRegistry `CCLZEBJX…4HWR`](https://stellar.expert/explorer/public/contract/CCLZEBJXG4YVJEPBCR5F27N733BCK5HQJWZZGB3K54JVODY3VAGP4HWR),
with WASM SHA-256 `982809197d35d44c7b0fce6bd117fb2fec09b728c64c146c1f803b01faacff62`.
See the [verification record](mainnet-v2-security-verification.md),
[data-flow diagrams](mainnet-v2-data-flow.md), and
[scan findings and dispositions](mainnet-v2-security-scan-report.md).

## Protected assets and assumptions

- User token balances and the registry's limited token allowance.
- Mandate integrity: user, agent, recipient, asset, budget, expiry, credential
  commitment, consumed amount, status, and sequence.
- The binding between a receipt, a successful token transfer, and consumed budget.
- Administrator authority, approved assets, pause state, schema, and implementation.
- The integrity of release source, dependency resolution, tests, and deployed bytes.

The contract relies on Stellar consensus and Soroban authorization, transaction
atomicity, storage, and token execution. The approved Circle USDC implementation
is trusted to implement token transfers correctly. Administrator asset admission
is a security decision; a callback probe does not establish that arbitrary tokens
have honest balance semantics. The user and agent must protect their own signing
authority. A user signature proves authorization, not that a compromised UI
explained the terms honestly.

SDKs, caches, RPC responses, the hosted UI, databases, AI output, x402 messages,
relay services, and merchants are not sources of on-chain spending authority.
The contract does not fetch x402 JSON, trust an AI decision, or consume a cached
validation result.

## Attack surfaces, controls, and executable evidence

Test names below refer to the deployed V2 crate's
[required executable manifest](../contracts/mainnet-v2/mandate-registry/tests.required).
Authorization failures are enforced by the Soroban host; they are not all typed
application errors. Rejection checks must satisfy other preconditions so an
unrelated error cannot disguise missing authorization.

| Threat | Enforced control | Representative executable evidence |
|---|---|---|
| Caller forges user registration or revocation | Stored user authorization; registration validates positive budget, bounded future expiry, and approved asset | `register_requires_user_auth`, `revoke_requires_user_auth`, `unknown_mandate_not_found` |
| Caller impersonates the agent, including through a contract extension | `execute_payment` requires the stored agent's authorization in the actual invocation tree | `execute_requires_agent_auth`, `contract_principals_authorize_only_their_own_nested_calls`, `hostile_extension::direct_caller_cannot_bypass_a_contract_agent` |
| Agent exceeds one-shot or cumulative budget, uses zero/negative amounts, or overflows arithmetic | Checked positive amount, stored-invariant validation, checked addition, exact-budget exhaustion | `overspend_single_rejected`, `overspend_cumulative_rejected`, `checked_spend_overflow_is_typed_and_atomic`, `all_invalid_stored_invariants_fail_closed`, `exhausted_status_then_rejected` |
| Replay or reordered/concurrent requests consume the same authorization twice | Current stored sequence must match; increment checked for overflow; transaction commits atomically | `replay_stale_seq_rejected`, `out_of_order_seq_rejected`, `sequence_exhaustion_is_typed_and_atomic`, `state_machine_runs_thousands_of_real_host_transitions` |
| Agent pays after expiry, revocation, exhaustion, pause, or asset removal | Money path re-reads and checks live state; expiry rejects at `timestamp >= expiry`; missing pause state is closed | `expired_mandate_rejected`, `revoked_mandate_rejected`, `pause_blocks_payment_without_changing_mandate_state`, `reviewed_asset_policy_is_enforced_on_registration_and_execution`, `missing_pause_state_fails_closed` |
| Agent substitutes recipient or asset | Execution takes neither recipient nor asset as caller-selected arguments; it uses stored terms. Preview checks supplied terms against those terms | `out_of_scope_merchant_rejected`, `validate_mandate_rejects_state_and_argument_mismatches`, `hostile_extension::hostile_extension_cannot_select_a_different_merchant_or_asset` |
| Reuse a credential or confuse domains | ID binds network, registry, user, agent, recipient, asset, budget, expiry, and credential commitment; per-user credential marker created with capped TTL prevents reuse while retained | `mandate_identifier_is_bound_to_registry_user_and_all_terms`, `credential_commitment_is_idempotent_across_changed_terms`, `duplicate_register_rejected` |
| Token allowance/balance failure leaves phantom spent budget or receipts | Budget, sequence, status, `transfer_from`, and event are in one reverting transaction | `insufficient_allowance_blocks_payment`, `exact_budget_token_failure_rolls_back_exhaustion`, `mandate_lifecycle_happy_path_moves_value_atomically` |
| Malicious token or extension finds a second money path | No independent extension allowance; stored checks repeated; bounded callback test exercises nested execution rejection | `reentry_probe::malicious_token_callback_is_bounded_and_atomic`, all five `hostile_extension` tests |
| Unauthorized governance, upgrade, or asset-policy change | Current administrator authorization; paused state additionally required for upgrade and policy edits | `admin_methods_require_authorization`, `wrong_contract_principals_cannot_use_governance_authority`, `upgrade_requires_pause_without_changing_state` |
| Attacker takes over administrator handoff | Current administrator proposes; proposed successor must authorize acceptance | `admin_rotation_transfers_control`, `wrong_contract_principals_cannot_use_governance_authority` |
| Upgrade silently resets mandate or incompatible state is treated as valid | Same-address preservation test; mandate paths require schema `2`; missing pause state blocks payments | `paused_admin_upgrade_replaces_wasm_at_same_address_and_preserves_storage`, `predecessor_schema_fails_closed_before_state_access`, `missing_schema_blocks_mandates_but_preserves_admin_recovery` |
| State expires or malicious TTL assumptions weaken enforcement | Bounded mandate lifetime, capped TTL extension, fail-closed missing state; public reads can refresh TTL but not spend | `active_contract_and_mandate_ttls_reach_the_reviewed_floor`, `mandate_lifetime_is_bounded_below_persistence_target` |
| Release tests are deleted, auth is mocked, bytes/interface drift, or dependency scan fails silently | Exact required-test list, no broad mock-authorization shortcuts, pinned toolchain/actions, locked interface and artifact, fail-closed dependency resolution | `scripts/gatecheck-contracts.sh`, `scripts/security-scan.sh`, `scripts/test-security-scan.sh`, continuous CI |

The large deterministic lanes exercise 10,001 consecutive signed amount values
plus extreme integers, and 512 complete mandate scenarios. These are test inputs,
not independent reviewers, exhaustive state-space coverage, or formal verification.

## Trust boundaries and deliberately excluded claims

1. **An agent can misuse an allowed budget.** The protocol limits spending terms;
   it does not judge whether a purchase is useful, factual, or aligned with the
   user's unstated intent. A rogue but authorized agent can spend within its cap.
2. **Recipient means the address stored in the mandate.** In the hosted relay
   flow that address can be the relay. Relay-to-marketplace payment and service
   delivery are separate operations. The registry does not enforce the ultimate
   seller, guarantee delivery, or atomically refund seller downtime. Two receipts
   must not be described as one atomic cross-service settlement.
3. **Preview is not a reservation.** `validate_mandate` is public and can become
   stale immediately. Successful preview is never payment proof; execution must
   still authorize the agent and validate the current stored state.
4. **Token approval is not a deposit.** Funds remain in the user's token balance.
   A registry allowance is token-level authority; the reviewed implementation
   limits its use via mandates. Other approvals or transfers signed independently
   by the user are outside this contract's protection.
5. **Public does not mean private.** Mandate terms, addresses, amounts, and events
   are on-chain. A credential commitment is not the original credential, but
   confidentiality is not guaranteed for low-entropy inputs. Do not put secrets
   or personal data directly into public mandate fields or shared reports.
6. **Archival is an availability boundary.** Ledger TTL is distinct from wall-clock
   mandate expiry. Refreshes are capped by network limits; restoration may require
   fees. The used-credential marker is not a promise of infinite retention.

## Governance and remaining risk

The deployed administrator is a native Stellar account with three distinct
weight-1 Ed25519 signers and thresholds `2/2/2`. The registry itself does not
implement quorum counting and does not force a proposed successor to be multisig.
Two compromised keys can change policy or install malicious code once paused.
Pausing is not an upgrade delay. V2 has no integrated timelock and no OpenZeppelin
access-control dependency; the older canary is a separate deployment.

Named custodian ownership, physical custody separation, rotation rehearsals, and
lost-key recovery require an operating record and human attestation. Public
account state proves key weights and thresholds, not who controls each key.
This contract test gate does not certify that private custody process. Two lost
keys have no hidden recovery mechanism supplied by V2.

Unknown defects, compromised tooling or maintainers, consensus/host defects,
future USDC behavior changes, and denial of service remain risks. The existing
host-only dependency maintenance exception is explicitly documented in the scan
record; it is not a remediated upstream dependency. Re-run these gates after
source, dependency, policy, or implementation changes. Future storage migration
needs a separate design, representative old-state tests, and fresh release proof.

This review supports a bounded technical readiness decision, not a guarantee of
zero future vulnerabilities or an endorsement from the Stellar Development Foundation.
