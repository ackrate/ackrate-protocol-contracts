#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export RUSTUP_TOOLCHAIN=1.98.0
export CARGO_TARGET_DIR="$PWD/contracts/admin/target"
controller=contracts/mainnet/timelock-controller/Cargo.toml
bash scripts/build-admin-test-inputs.sh
cargo fmt --manifest-path contracts/admin/Cargo.toml --all -- --check
cargo fmt --manifest-path "$controller" --all -- --check
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
export SOROBAN_SDK_BUILD_SYSTEM_SUPPORTS_SPEC_SHAKING_V2=1
cargo clippy --manifest-path contracts/admin/Cargo.toml --locked --all-targets -- -D warnings
cargo clippy --manifest-path "$controller" --locked --all-targets -- -D warnings
cargo test --manifest-path "$controller" --locked

if [[ "${1:-}" == "--with-deployed-v2" ]]; then
  curl --fail --silent --show-error --location --output contracts/admin/target/mainnet-v2.wasm \
    https://github.com/ackrate/ackrate-protocol-contracts/releases/download/v2-source-verify-v0.4.1.6_mandate-registry_pkg0.4.1_cli27.0.0/mandate-registry_v0.4.1.wasm
  echo '982809197d35d44c7b0fce6bd117fb2fec09b728c64c146c1f803b01faacff62  contracts/admin/target/mainnet-v2.wasm' | shasum -a 256 --check --status
  cargo clippy --manifest-path "$controller" --locked --all-targets --features deployed-v2-test -- -D warnings
  cargo test --manifest-path "$controller" --locked --features deployed-v2-test
fi
node --check tools/admin/admin.mjs
node --check tools/admin/proof.mjs
node --check tools/admin/deployment.mjs
node --check tools/admin/transfer-demo.mjs
