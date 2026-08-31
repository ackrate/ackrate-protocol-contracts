# Mainnet v2 MandateRegistry

[![Contract Gate Check](https://github.com/ackrate/ackrate-protocol-contracts/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/ackrate/ackrate-protocol-contracts/actions/workflows/ci.yml?query=branch%3Amain)

## Status

This repository's active milestone is **T3 Step 1**.

`0.4.1` is ACKRATE's condensed MandateRegistry review candidate on `main`.
It is not deployed to Stellar mainnet. A secret-free preparation workflow can
build and inspect unsigned Mainnet transactions, but it cannot sign anything
and broadcasting requires a separate hash-bound confirmation. V2 releases use
one dedicated source-verification route, and `contracts/simple` remains unchanged.

V2 is one contract with one payment path. The contract—not the SDK, agent,
cache, x402 adapter, or merchant server—is the enforcement layer.

> [!IMPORTANT]
> **Evidence-backed release posture:** every confirmed finding from eight
> completed review rounds is repaired, its regression is locked where
> applicable, and the complete local gate is green. This is strong release
> evidence, not a claim that unknown defects are mathematically impossible.

## Why V2 is smaller and sharper

V2 was rebuilt from the proven Simple behavior without changing
`contracts/simple`. The refactor follows Soroban and Stellar primitives instead
of adding framework layers:

| Design choice | What it achieves |
|---|---|
| One `MandateRegistry` contract | No timelock contract, upgrade queue, plugin system, or second settlement route. |
| Enforcement centered in `lib.rs` | The complete authorization, budget, sequence, expiry, asset, merchant, transfer, and receipt path is readable in one place. |
| Storage isolated in `storage.rs` | One schema owner, bounded persistent entries, explicit TTL renewal, and no caller-sized collection traversal. |
| Small typed boundaries | `error.rs`, `types.rs`, and `events.rs` contain only stable errors, durable values, and contract-spec events. |
| Native Soroban authorization | Users, agents, and the administrator must authorize through contract principals; the SDK is never trusted as authority. |
| Atomic SEP-41 settlement | State consumption, token transfer, and the typed receipt succeed together or roll back together. |
| Wire-format independence | x402 request shapes stay outside contract storage and the public mandate interface, so the adapter can evolve separately. |
| Paused, native upgrade path | Same-address WASM replacement requires the 2-of-3 administrator and an already-paused contract. |

The result is a five-file production kernel with one money-moving function,
18 locked public functions, 9 typed events, and no alternate payment surface.

## 1,000-Review Campaign: Completed Evidence

### What ACKRATE actually used

- **24 independent agent reviewers across 8 completed rounds.**
- A campaign designed to continue toward **1,000 independent reviews**; 1,000
  is the target, not the completed-agent count.
- **12,561 deterministic Soroban host checks/actions** in every full V2 run.
- **49 required executable tests:** 48 native-host tests and one direct smoke
  of the exact optimized contract file.
- A closed loop for every confirmed finding: reproduce it, repair it, add a
  permanent regression where applicable, rerun the entire gate, and publish
  the exact repaired revision to `main`.

The 12,561 figure counts executable cases and state actions, not agents.

> [!WARNING]
> **Cycle 9 release-identity finding — deployment blocked and repaired**
> The contract behavior suite passed, but the GitHub release gate rejected the
> candidate because the fingerprint recorded from a macOS build did not match
> the pinned Linux release build. No contract was uploaded, deployed, or signed.
> Deployment is now locked to the exact GitHub Linux artifact used by the
> StellarExpert source-verification release; local builds may verify behavior,
> size, and interface, but cannot substitute their bytes for that artifact.

| What the release gate caught | What we changed |
|---|---|
| The recorded file fingerprint came from the wrong build platform, even though the contract tests and public interface matched. | The canonical fingerprint now comes only from the pinned GitHub Linux build. Upload preparation rejects every other file, and deployment remains blocked until that exact release artifact is downloaded and verified. |

> [!NOTE]
> **Live Mainnet rehearsal finding — fixed before contract upload**
> Stellar CLI 27.0.0 requires an RPC URL value while calculating an otherwise
> local transaction hash. The helper now supplies a deliberately non-routable
> `.invalid` URL, so hashing stays local and fails closed if the CLI ever tries
> to use the network. The release artifact now also carries the source-repository
> and home-domain metadata required for StellarExpert source verification.

> [!NOTE]
> **Latest completed cycle — cycle 8**
> Contract source: unchanged from cycle 7 and the full gate remains green.
> Deployment controls: one issue found and fixed. Documentation accuracy: two
> issues found and fixed. The repaired revision passed 12,561 deterministic
> checks/actions and all 49 required tests: 48 native-host tests plus one
> direct execution smoke of the exact optimized release file.

| Proof at a glance | Completed result |
|---|---:|
| Independent agent reviewers | 24 across 8 rounds |
| Contract-source changes in cycle 8 | 0 |
| Deployment-control issues in cycle 8 | 1 found / 1 fixed |
| Documentation issues in cycle 8 | 2 found / 2 fixed |
| Deterministic cases/actions per full run | 12,561 |
| Required executable tests | 48 native + 1 exact optimized-WASM smoke |
| Public interface locked | 18 functions / 9 events |
| Exact optimized contract exercised | Yes |

### Why this process is different

ACKRATE is treating agentic review as a cumulative engineering system, not a
one-time report. Independent agent reviewers challenge a fixed revision. A
result enters this record only after it is reproduced. Every confirmed issue is
fixed in code, the release gate, or the public evidence; contract and release defects
become permanent regression checks. The exact compiled contract is exercised,
the repaired revision is published to `main`, and a new review round begins.

The differentiator is the closed loop and its public evidence: **discover →
reproduce → repair → lock the regression → execute the release bytes → publish
the result → repeat**.

| Campaign evidence | Current result |
|---|---:|
| Independent agent reviewers completed | 24 across 8 rounds |
| Executable contract checks/actions per full run | 12,561 |
| Required executable tests | 49 |
| Confirmed issues in the latest cycle | 3 found / 3 fixed |
| Release artifact exercised directly | Yes |

### Latest findings and repairs

| What the agents caught | What we fixed |
|---|---|
| A two-signature transaction could pass the broadcast guard without proving it was the same unsigned transaction the team reviewed. | Submission now requires both files, rejects a signed “unsigned” file, and proves both transaction hashes are identical before broadcast. |
| The public deployment helper used named team roles while the README said V2 custodian identities stay private. | Public inputs and messages now use neutral signer numbers; names and real V2 signer addresses remain private. |
| The README described recovery procedures that the simplified private signing guide no longer contains. | The README now separates the five-transaction signing guide from the still-required private custody, rotation, and loss-response record. |

Separately, ACKRATE maintains a private five-transaction signing guide for the
named 2-of-3 team. V2 custodian identities, V2 signer addresses, and the private
V2 playbook are not kept in this public repository. A separate written custody,
rotation, and loss-response record remains a deployment requirement.

<details>
<summary><strong>Cycle 7 documentation fixes</strong></summary>

| What the agents caught | What we fixed |
|---|---|
| The cycle summary read as if the optimized-WASM smoke ran in addition to 49 tests. | The summary now states the exact total: 48 native-host tests plus one optimized-WASM smoke equals 49. |
| The V2 README said no wallet address existed anywhere in the public repository, but older contract-family deployment records contain public governance addresses. | The privacy statement is now precise: V2 custodian identities, V2 signer addresses, and the private V2 playbook are not published here. |

</details>

<details>
<summary><strong>Cycle 6 evidence-wording fix</strong></summary>

| What the agents caught | What we fixed |
|---|---|
| The gate log implied that every test loaded the optimized WASM, while 48 tests exercise the native contract and one separately loads the exact optimized file. | The public record and gate now state the evidence precisely: 48 native-host tests plus one exact optimized-WASM execution smoke. |

</details>

<details>
<summary><strong>Cycle 5 release-gate fixes</strong></summary>

| What the agents caught | What we fixed |
|---|---|
| The action-pin rule blocked a few familiar mutable names but could miss a different branch name or a shortened commit. | Every external action reference must now be a complete immutable commit digest; a permanent fixture challenges it with branches, tags, short commits, and unpinned containers. |
| A future feature-gated test could appear in the required list without running. | The gate now executes the entire all-feature manifest, including ignored tests, and separately executes the exact optimized release file. |

</details>

<details>
<summary><strong>Cycle 4 release-gate fixes</strong></summary>

| What the agents caught | What we fixed |
|---|---|
| A V2-looking release tag could publish an older contract family. | Release tags now pass through one unambiguous router. V2 release tags fail closed until a dedicated V2 release path is separately approved. |
| A required test could keep its name but be silently marked “ignored.” | Every required test is now executed even if it is marked ignored; names alone are not enough. |
| Different build machines produced different file fingerprints from the same source. | The release fingerprint is now defined by the pinned Linux release environment and Stellar CLI 27.0.0; local machines still verify behavior, size, and interface. |

</details>

<details>
<summary><strong>Cycle 3 fixes from 0.4.1</strong></summary>

| What the agents caught | What we fixed |
|---|---|
| Old and new data could be mistaken for the same version, creating a duplicate-spending risk after a bad upgrade. | Old data now stops safely before any mandate or money can be touched. |
| The release job was missing the Stellar build tool. | Clean release machines now install and verify the exact required tool. |
| The gate checked function names, but not the exact compiled contract file. | The exact file size and fingerprint are now locked. |
| Tests did not run the actual compiled file that would be shipped. | The gate now loads that exact file and proves sign-in, payment, rollback, state, and receipts. |
| Important tests could be removed without the gate noticing. | The gate now requires the complete 49-test list. |
| One callback test claimed more than it proved. | The claim now matches the proof: a hostile callback cannot spend twice or advance twice. |
| The state diagram missed one real user action. | The diagram and permanent test now match the contract. |

</details>

<details>
<summary><strong>Earlier fixes from 0.4.0</strong></summary>

| What the agents caught | What we fixed |
|---|---|
| A fake token could pretend a payment succeeded. | Only reviewed assets are accepted, and every receipt names the asset. |
| Another user could claim a public credential hash first. | Every mandate ID is bound to its network, contract, user, parties, asset, budget, and expiry. |
| One credential could create several budgets. | One user credential can create only one budget. |
| A mandate could outlive its storage window. | Mandates end within 30 days while active state targets roughly 120 days. |
| A bad administrator address could permanently receive control. | Control changes require proposal plus acceptance by the new administrator. |
| Payment receipts were missing asset and sequence. | Receipts now identify both. |
| Failed-payment checks did not prove every field rolled back. | Balance, budget, status, sequence, and receipts now roll back together. |
| Damaged or incompatible state could be read as valid. | State is validated before any settlement. |

</details>

The campaign continues through repeated independent rounds toward the
1,000-review target. The number 12,561 is the deterministic case/action count,
not a reviewer count. Counts above state completed work only; unconfirmed
suggestions are excluded.

```mermaid
flowchart LR
    A["Independent agent reviewers"] --> B["Reproduce the finding"]
    B --> C["Fix the contract or release gate"]
    C --> D["Add permanent regression evidence"]
    D --> E["Run 12,561 cases/actions + full repo gate"]
    E --> F["Push exact SHA to main"]
    F --> A
```

## Contract in one view

The production contract is intentionally five files:

```text
mandate-registry/src/
├── lib.rs       # public API and complete enforcement path
├── storage.rs   # all storage keys, schema, and TTL policy
├── error.rs     # stable typed errors
├── types.rs     # durable mandate and identifier types
└── events.rs    # typed contract-spec events
```

Test-only hostile caller and callback contracts do not enter release WASM.
There is no timelock controller, upgrade queue, scheduler, cancellation path,
plugin hook, extension callback, or second payment function.

```mermaid
flowchart LR
    U["User signer"] -->|"register / revoke"| R["MandateRegistry"]
    A["Agent + SDK\nuntrusted"] -->|"id, amount, sequence"| R
    X["x402 adapter\nreplaceable"] --> A
    R -->|"stored user, merchant, asset, amount"| T["Reviewed SEP-41 asset"]
    T --> M["Stored merchant"]
```

## The enforcement kernel

`execute_payment` is the only money-moving entry point. A successful call must
prove, in one Soroban invocation:

1. schema `2` is present and the contract is not paused;
2. the mandate exists and its stored agent authorizes the exact call;
3. the supplied sequence matches and can advance;
4. amount, stored status, spend, budget, and expiry are valid;
5. the stored asset is still approved and the stored merchant is used;
6. checked arithmetic keeps cumulative spend within budget;
7. updated spend, sequence, and status are written;
8. the reviewed SEP-41 asset transfers from the stored user to merchant; and
9. a typed payment receipt is emitted.

Any authorization, state, event, or token failure rolls back the whole
invocation. The caller cannot choose the user, merchant, asset, or agent during
settlement.

```mermaid
sequenceDiagram
    participant A as Agent / SDK
    participant R as MandateRegistry
    participant S as Stored mandate
    participant T as Reviewed asset
    A->>R: execute_payment(id, amount, sequence)
    R->>S: read current state
    R->>R: authorize + enforce every invariant
    R->>S: consume budget and sequence
    R->>T: transfer_from(user, merchant, amount)
    R-->>A: typed receipt
    Note over R,T: Any failure reverts state, tokens, and event.
```

### Mandate state

```mermaid
stateDiagram-v2
    [*] --> Active: user-authorized registration
    Active --> Active: payment below budget
    Active --> Exhausted: payment reaches budget
    Active --> Revoked: user revokes
    Exhausted --> Revoked: user records withdrawal after exhaustion
    Revoked --> Revoked: payment rejected
    Exhausted --> Exhausted: payment rejected
```

Expiry is checked from ledger time on every validation and execution. It is not
stored as a mutable status.

## Public surface

| Area | Functions | Rule |
|---|---|---|
| Initialize | `__constructor` | Writes schema `2`, final administrator, running state, and first reviewed asset at fresh creation. |
| Mandates | `derive_mandate_id`, `register_mandate`, `get_mandate`, `validate_mandate`, `execute_payment`, `revoke_mandate` | User creates/revokes; agent executes; validation is advisory; execution always rechecks. |
| Emergency | `pause`, `unpause`, `is_paused` | Pause blocks the only money path; recovery and revocation remain available. |
| Assets | `set_asset_allowed`, `is_asset_allowed` | Administrator changes policy only while paused; removal blocks existing mandates. |
| Authority | `get_admin`, `get_pending_admin`, `propose_admin`, `accept_admin` | Two-step handoff prevents accidental loss of control. |
| Compatibility | `get_schema_version`, `upgrade` | Mandate methods reject non-current storage; same-address upgrades require administrator authorization and pause. |

There are exactly 18 exported functions and 9 typed event schemas. The gate
fails if either surface changes without an explicit reviewed update.

## Storage, identity, and trust boundaries

- `storage.rs` is the only production module that calls `env.storage()`.
- Schema `2` is deliberately incompatible with predecessor schema `1`; this
  blocks mixed-layout execution rather than guessing at a migration.
- One persistent entry holds each mandate. No caller-sized collection is
  stored or traversed.
- A mandate ID is the SHA-256 of an independently versioned domain plus network,
  registry, user, agent, merchant, asset, maximum, expiry, and credential hash.
- `UsedCredential(user, vc_hash)` prevents the same approval from producing
  multiple budgets.
- A mandate may live at most 30 days. Instance and persistent state are bumped
  toward a roughly 120-day target, clipped to the network maximum.
- The asset allowlist is a governance trust root. Production policy must admit
  only canonical, independently verified Stellar Asset Contract addresses.
- x402 wire types never enter storage or the public interface. Future x402
  adapters can change without redesigning the enforcement contract.

The SDK, adapters, caches, agents, UI, RPC provider, and merchant server are
untrusted. `validate_mandate` is only a preview; it cannot authorize or consume
anything. Settlement systems must verify network, registry, asset, mandate ID,
and sequence—not merely the presence of an event.

## Administration without a timelock

The contract has no timelock. `upgrade` calls Soroban's native same-address
WASM replacement only after current-administrator authorization and an
already-paused state.

The private signing guide explains the five Mainnet transactions and the
two-person Freighter handoff in plain language. Custody separation, signer
rotation, loss response, and the two-key-loss boundary require a separate
private governance record before deployment.

Production policy is a native Stellar 2-of-3 account at the administrator
address. Deployment remains blocked until the private authority record names
all three public signers, organizational holders, separate custody locations,
rotation steps, and one-key-loss recovery. Two lost keys have no hidden
recovery path. V2 custodian identities, V2 signer addresses, and its private
signing guide do not belong in this public directory.

Version `0.4.1` is **fresh-create only**. It must be constructed at a new
contract address and must not be installed over schema `1` predecessor state.
The code also enforces this boundary: mandate reads, writes, and payments reject
any schema other than `2`. A future compatible upgrade must preserve schema
`2`; a future layout change requires an explicit migration and representative
old-state tests.

## Executable gate

From the repository root:

```bash
./scripts/gatecheck-contracts.sh
./scripts/security-scan.sh
```

The gate performs strict formatting and linting, runs every contract suite,
executes the V2 high-volume lane in the Soroban host, builds optimized
`wasm32v1-none`, checks the complete interface, and enforces the reviewed WASM
size/hash. The dependency policy rejects known vulnerabilities, yanked crates,
unexpected warnings, and accepted host-only code entering deployed WASM.

### Current V2 evidence

| Evidence | Current result |
|---|---:|
| Independent agent reviewers completed across eight rounds | 24 |
| Amount/expiry boundary cases | 10,001 |
| Authenticated state-machine actions | 2,560 |
| Deterministic host cases/actions per full V2 gate | 12,561 |
| Executable tests | 48 native-host + 1 exact optimized-WASM smoke |
| Exported functions / typed events | 18 / 9 |

The tests use executable Soroban contract principals and a registered Stellar
Asset Contract for positive paths. V2 source contains no authorization-bypass
helper or dummy fixture. Continuous negative coverage includes unauthorized
callers, expired mandates, single and cumulative overspend, replay and ordering,
revocation, pause, asset and merchant binding, failed-transfer rollback,
predecessor/missing schema, hostile callers, callback attempts, unsafe upgrades,
event XDR, and TTL floors.

Exact artifact facts are updated only after the repaired gate completes:

| Artifact | Reviewed value |
|---|---|
| Optimized WASM size | 15,510 bytes |
| Canonical Linux WASM SHA-256 | `982809197d35d44c7b0fce6bd117fb2fec09b728c64c146c1f803b01faacff62` |
| Canonical full-interface SHA-256 | `69c201ce1fb089ccfef06f125826b0aeba72af1b1536cb0b19e8cb05970ee805` |
| Embedded source metadata | `github:ackrate/ackrate-protocol-contracts` |
| Embedded home domain | `ackrate.xyz` |

## Guarded Mainnet preparation

`scripts/deploy-mainnet-v2.sh` prepares the account setup, upload, and fresh
deployment without accepting or storing any signing secret. Its four
preparation paths are build-only. The workflow enforces:

- one reviewed account as source, 2-of-3 authority, and contract admin;
- exactly three distinct weight-1 signers with thresholds `2/2/2`;
- the reviewed V2 byte hash and canonical Circle Stellar Mainnet USDC SAC;
- exactly two distinct signatures for upload and deployment;
- an exact match between the signed XDR and its reviewed unsigned XDR;
- a separate confirmation containing the exact Mainnet transaction hash; and
- read-only post-deployment checks for code hash, admin, pending admin, schema,
  initial pause state, and asset policy.

The workflow cannot sign. CI publishes the exact canonical candidate only from
a push to `main`, after both dependency security and the complete contract gate
pass. The dedicated `mainnet-v2-v*` route uses the commit-pinned official
StellarExpert build workflow to create the attested release used for deployment.
No step automatically deploys.

## Remaining release blockers

Passing tests cannot prove the absence of unknown defects. Before any V2
contract upload or deployment, all of these remain mandatory:

- finish repeated independent review, property/fuzz, mutation, differential,
  and resource-cost lanes;
- complete and rehearse the named 2-of-3 custody and recovery record;
- reconcile the supplied T3 Step 1 OpenZeppelin/timelock wording with the
  explicitly no-timelock V2 governance profile before claiming that acceptance
  line;
- verify reference apps teach mandate registration plus authoritative on-chain
  execution and warn against agent token allowances or cached approval; and
- run testnet drills for rogue-agent spending within budget, merchant downtime
  after settlement, and mid-flow expiry, preserving transaction evidence and
  user-visible outcomes.

No claim of perfection, immutability, or readiness for billion-dollar custody
is made by this candidate.

## Primary Stellar references

- [Contract authorization](https://developers.stellar.org/docs/build/guides/auth/contract-authorization)
- [Storage strategies](https://developers.stellar.org/docs/build/guides/storage/storage-strategies)
- [Storage migration](https://developers.stellar.org/docs/build/guides/storage/migrate-contract-storage)
- [Typed events](https://developers.stellar.org/docs/build/smart-contracts/example-contracts/events)
- [Same-address upgrades](https://developers.stellar.org/docs/build/guides/conventions/upgrading-contracts)
- [Cost and size analysis](https://developers.stellar.org/docs/build/guides/fees/analyzing-smart-contract-cost)
- [Differential testing](https://developers.stellar.org/docs/build/guides/testing/differential-tests)
- [Fuzz testing](https://developers.stellar.org/docs/build/guides/testing/fuzzing)

## Deployment boundary

This work performs no Stellar upload, install, invoke, deployment, or live-state
mutation. Any future deployment is a separate, explicitly authorized release.
