# Legacy protocol development contract

This is the unchanged contract tree moved from
[`ackrate-protocol` at `9a41cfa`](https://github.com/ackrate/ackrate-protocol/tree/9a41cfa/contracts/mandate-registry).
It originated in the protocol monorepo and includes the historical composite
payment implementation, tests, snapshots, and lockfile. It differs from the
maintained composite variant; no files were overlaid on that implementation.

This is not the deployed Mainnet V2 source. See
[Mainnet V2](../mainnet-v2/README.md) for deployment identity and provenance.
Keep this variant for historical testnet reproduction; new contract work belongs
in the appropriate maintained variant. Its Rust checks run in Contract Gate Check.

```sh
cd contracts/legacy-protocol/mandate-registry
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

The protocol's testnet deployment helper accepts `ACKRATE_CONTRACTS_ROOT` pointing
to this repository and explicitly builds this variant. It does not deploy V2.
