# Reference mandate extension

This contract demonstrates the smallest safe general extension profile for the
Simple MandateRegistry.

The registry stays extension-unaware. A user registers an ordinary mandate
whose `agent` is this extension, then installs one immutable policy containing:

- one executor;
- a per-payment cap below the registry mandate budget; and
- a start time.

Every execution also carries a short validity window, the expected registry
sequence, and a one-time nonce. The extension checks its stricter policy and
then calls the unchanged registry. MandateRegistry still performs the
authoritative status, expiry, merchant, asset, budget, pause, replay, allowance,
state-consumption, and token-transfer work.

## Deliberate limits

- no administrator or global allowlist;
- no upgrade or policy replacement;
- no arbitrary rule interpreter or unbounded proof bytes;
- no extension chaining;
- no token import, token call, balance custody, or allowance;
- no registry callback or hook; and
- no fallback around `MandateRegistry.execute_payment`.

A malicious user-selected extension can ignore its advertised extra policy and
spend inside the base mandate. It still cannot exceed the base mandate because
the registry remains the only money path. Users must therefore keep the base
merchant, asset, total budget, and expiry tight.

This reference is not a mainnet deployment claim. The policy lifetime, storage
TTL, resource ceiling, optimized WASM, registry address, and network
configuration remain release gate checks.

## Gate check

```bash
cargo fmt --manifest-path contracts/extensions/reference-policy/Cargo.toml --all -- --check
cargo clippy --manifest-path contracts/extensions/reference-policy/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path contracts/extensions/reference-policy/Cargo.toml
cargo build --manifest-path contracts/extensions/reference-policy/Cargo.toml --target wasm32v1-none --release
```
