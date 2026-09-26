# Atomic wallet setup

A separate immutable helper registers a mandate on the existing V2 registry and
approves the exact capped token allowance in one Soroban invocation. It neither
holds funds nor takes over the registry. The existing registry is unchanged.

The constructor fixes the registry and asset. There is no administrator, routing
setter, generic execution method or upgrade entry point. `get_config` exposes
those two targets. `register_and_approve` requires the user's authorization of
its complete arguments and the registry/token child invocations. Registration
and allowance either both succeed or both revert. The spender is always the
registry, never the helper or agent. Setup does not transfer tokens.

The allowance is positive and expires within 17,280 ledgers. Mandate expiry and
all existing registry admission/payment checks remain enforced by the registry.
A new setup replaces that wallet's allowance to this registry with the reviewed
cap, as the previous two-transaction flow did. It does not add to that cap.

## Reproduce

Use Rust 1.98.0 with `wasm32v1-none`, clippy and rustfmt:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release --target wasm32v1-none
```

Run these in this directory. The separate lockfile uses the existing reviewed
registry dependency baseline; do not regenerate it against arbitrary latest
transitive versions. Tests execute the actual V2 registry and Stellar Asset
Contract, including exact nested authorization and registration rollback when
the token approval is unauthorized.

## Deployment boundary

No Mainnet address is claimed by this source. Before enabling a deployment in
the wallet, record its network, contract ID, exact WASM hash, constructor targets
and successful atomic-setup receipt. The wallet verifies the helper bytecode and
constructor targets before asking for a signature. Keep the release disabled
until those checks pass. Immutable helpers still need ledger TTL maintenance or
restoration; an unavailable helper must fail before signing, not redirect calls.

The Testnet runner in the demo repository deploys this WASM with the exact
reviewed V2 registry bytecode in an isolated Testnet instance and uses test XLM.
That does not establish Mainnet USDC success or Freighter popup acceptance.
