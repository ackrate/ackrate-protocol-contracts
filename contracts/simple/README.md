# Simple MandateRegistry

The Simple contract is ACKRATE's minimal Soroban mandate and payment registry.
It supports mandate registration, inspection, revocation, scoped SEP-41 payments,
an emergency pause, admin rotation, and direct same-address WASM replacement.

This directory is independent from the Composite and governed Mainnet variants.

## Source layout

Production logic intentionally lives in two files:

- `mandate-registry/src/lib.rs` contains the public interface, types, errors,
  authorization, validation, payment, administration, and events.
- `mandate-registry/src/storage.rs` is the only production file that reads or
  writes contract storage and manages persistent mandate TTL.

`test.rs`, `reentry_probe.rs`, and `hostile_extension.rs` are test-only files.

## Administration and upgrades

The constructor stores one admin `Address`. That address authorizes `set_admin`,
`pause`, `unpause`, and `upgrade` through Soroban `require_auth()`.

```rust
upgrade(new_wasm_hash: BytesN<32>)
```

`upgrade` immediately calls `update_current_contract_wasm`, preserving the
contract ID and storage. Simple has no `schedule_upgrade`, `cancel_upgrade`,
pending-upgrade storage, fixed delay, or pause requirement. For mainnet the
admin is a Stellar G-account configured with three weight-1 keys and threshold
2, so any two signers authorize the transaction. See
[`docs/DEPLOYMENT.md`](../../docs/DEPLOYMENT.md).

## Interface

| Method | Authorization | Purpose |
| --- | --- | --- |
| `__constructor(admin)` | deployment | Establish the initial admin and unpaused state. |
| `get_admin()` | none | Read the admin. |
| `set_admin(new_admin)` | current admin | Rotate authority. |
| `pause()` / `unpause()` | current admin | Stop or restore payments. |
| `is_paused()` | none | Read pause state. |
| `upgrade(new_wasm_hash)` | current admin | Replace WASM at the same contract ID. |
| `register_mandate(...)` | user | Store a bounded mandate. |
| `validate_mandate(...)` | none | Dry-run payment validation. |
| `execute_payment(...)` | mandate agent | Atomically consume budget and transfer tokens. |
| `revoke_mandate(id)` | mandate user | Withdraw consent. |
| `get_mandate(id)` | none | Read a mandate. |

## Invariants

- `execute_payment` is the only money-moving path.
- The agent is authenticated before consumption.
- Sequence, expiry, status, merchant scope, positive amount, and cumulative
  budget are checked against stored state on every payment.
- State consumption and SEP-41 `transfer_from` are atomic; a token failure
  reverts both.
- Registration initializes `spent = 0`, `seq = 0`, and `status = Active`.
- Pause blocks payments without blocking reads, registration, or revocation.

## Checks

```sh
cargo fmt --manifest-path contracts/simple/mandate-registry/Cargo.toml -- --check
cargo clippy --manifest-path contracts/simple/mandate-registry/Cargo.toml \
  --all-targets -- -D warnings
cargo test --manifest-path contracts/simple/mandate-registry/Cargo.toml
cargo build --locked --release --target wasm32v1-none \
  --manifest-path contracts/simple/mandate-registry/Cargo.toml
```
