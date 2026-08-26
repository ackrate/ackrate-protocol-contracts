# Mainnet security verification report

Status: passing release evidence
Run date: 2026-08-27

This report is versioned by the commit that contains it. Re-run
`./scripts/security-scan.sh` and the contract gate before relying on a later
commit.

## Results

| Check | Result |
|---|---|
| MandateRegistry negative and integration suite | Pass: 15 tests |
| TimelockController suite | Pass: 8 tests |
| Mainnet security total | Pass: 23 tests |
| Rust formatting | Pass |
| Clippy with warnings denied | Pass |
| Mainnet WASM target build | Pass |
| Dependency vulnerabilities | Pass: 0 known vulnerabilities in both Mainnet lockfiles |
| Yanked dependencies | Pass: 0 in both Mainnet lockfiles |
| Deployed-WASM dependency policy | Pass: the accepted host-only maintenance advisory is absent from both `wasm32v1-none` graphs |
| Source / artifact / chain identity | Pass: exact hashes and constructor/state checks recorded in `contracts/mainnet/deployment-manifest.json` |

Tools used for this recorded run: `rustc 1.98.0`, `cargo 1.98.0`, and
`cargo-audit 0.22.2`. Release artifacts remain governed by the separate pinned
toolchain and exact-byte workflow in the deployment manifest.

Lockfile identities after remediation:

```text
ff3131bc3909bbf488791be572940b52a15b9f3be2849329ffbf161931dbf319  contracts/mainnet/mandate-registry/Cargo.lock
ce209c5ce0f5a5105acf7f6116762c27caec5a61d9f27c941f60595df9a97d66  contracts/mainnet/timelock-controller/Cargo.lock
```

## Findings and disposition

| Finding | Initial state | Disposition |
|---|---|---|
| `RUSTSEC-2026-0009` in `time 0.3.36` (stack exhaustion denial of service) | Present in the MandateRegistry host/test lock graph | Remediated: updated to `time 0.3.47`; audit now reports zero vulnerabilities. |
| Yanked `spin 0.9.8` | Present in the MandateRegistry host/test lock graph | Remediated: updated to `spin 0.9.9`; audit now reports zero yanked crates. |
| `RUSTSEC-2024-0436` (`paste 1.0.15` unmaintained) | Transitive Soroban host/test dependency in both locks; no patched compatible release is available | Mitigated and explicitly gated: `paste` is absent from both deployed `wasm32v1-none` graphs. CI fails if this is no longer true or if any additional warning appears. Track upstream Soroban dependency removal. |

There are no unresolved findings in deployed contract code. The single
accepted maintenance advisory is confined to test/host tooling, is not linked
into either Mainnet WASM, and is enforced as an exact allowlist—not a general
warning exemption.

## Reviewer reproduction

```bash
# Installs the scanner used by CI if it is not already available.
cargo install cargo-audit --version 0.22.2 --locked

./scripts/security-scan.sh

cargo fmt --manifest-path contracts/mainnet/mandate-registry/Cargo.toml --all -- --check
cargo clippy --manifest-path contracts/mainnet/mandate-registry/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path contracts/mainnet/mandate-registry/Cargo.toml

cargo fmt --manifest-path contracts/mainnet/timelock-controller/Cargo.toml --all -- --check
cargo clippy --manifest-path contracts/mainnet/timelock-controller/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path contracts/mainnet/timelock-controller/Cargo.toml
```

For the exact canonical Ubuntu release bytes, interface checks, provenance, and
deployment comparison, run `./scripts/gatecheck-mainnet.sh` in the documented
release environment or inspect the completed deployment manifest and linked CI
runs.
