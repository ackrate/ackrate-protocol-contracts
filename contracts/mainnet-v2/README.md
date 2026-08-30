# Mainnet v2 contract refactor

## Status

The T2 design is approved. This folder currently records the implementation
contract for the refactor; contract code and release evidence will be added
next. Nothing in `contracts/mainnet-v2` has been deployed to Stellar mainnet.

All work lands directly on `main`. The live governed canary and its evidence
remain under [`contracts/mainnet`](../mainnet) and are not modified by this
refactor.

## Objective

Refactor the mainnet contract into a smaller, reviewable implementation without
weakening the current authorization, payment, pause, or upgrade guarantees. The
result must keep one atomic path from mandate validation through mandate
consumption to token transfer, with failure rolling back the entire operation.

## Planned layout

The implementation will live entirely under this folder and separate concerns
without creating alternate execution paths:

```text
contracts/mainnet-v2/
├── README.md
├── mandate-registry/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── types.rs
│       ├── error.rs
│       ├── storage.rs
│       ├── authorization.rs
│       ├── mandate.rs
│       ├── payment.rs
│       ├── admin.rs
│       ├── upgrade.rs
│       ├── events.rs
│       └── test.rs
└── timelock-controller/
```

Module boundaries are internal organization only. They must not introduce
extension registries, plugin hooks, callbacks, or a second payment route.

## Required behavior

- Use Soroban native address authorization for every privileged or payer action.
- Keep validation, replay protection, state consumption, and token transfer in
  one transaction.
- Reject expired, revoked, replayed, malformed, wrong-asset, and overspending
  mandates before committing state.
- Keep the verified Stellar mainnet USDC contract as the only initially allowed
  asset.
- Preserve the emergency pause boundary: money movement stops while safe reads
  and governance recovery remain available.
- Keep upgrades governed by the reviewed timelock and role separation; there is
  no bootstrap administrator or bypass path.
- Bound storage TTL by mandate expiry and use checked arithmetic for ledger,
  amount, and time calculations.
- Version persistent storage explicitly. Any layout change requires a migration
  path and tests against representative existing records.
- Emit stable, documented events for mandate lifecycle, payments, role changes,
  pause state, and upgrades.
- Avoid logging secrets, signed payloads, or unnecessary customer data.

## Security and release gate

Implementation is complete only when all of the following pass from a clean
checkout:

- formatting and lint checks with warnings denied;
- unit, integration, authorization, rollback, replay, expiry, overspend,
  re-entry, pause, role-rotation, timelock, upgrade, and migration tests;
- optimized `wasm32v1-none` builds for both contracts;
- interface and WASM size inspection;
- dependency and security scans;
- deterministic artifact hashing and source-provenance verification; and
- a final diff review proving no deployment command or live contract change is
  included.

The repository gate will be extended so `contracts/mainnet-v2` is checked on
every push to `main` before any release tag can be created.

## Delivery order

1. Scaffold the isolated v2 crates and pin the reviewed toolchain and
   dependencies.
2. Move types, errors, storage, and events behind explicit module boundaries.
3. Move authorization, mandate lifecycle, and the atomic payment path without
   changing their external guarantees.
4. Add governance, pause, timelock, upgrade, and storage-migration coverage.
5. Add the v2 crate to the repository security and release gate.
6. Review the complete diff, hashes, and evidence on `main`.

## Explicit non-goals

- No Stellar mainnet deployment.
- No mutation of the live canary or its deployment manifest.
- No new asset, extension, callback, or plugin surface.
- No SDK, web application, or off-chain service changes in this refactor.
- No release tag until the full gate and independent review pass.
