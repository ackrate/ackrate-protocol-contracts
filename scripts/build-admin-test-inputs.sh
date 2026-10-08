#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export RUSTUP_TOOLCHAIN=1.98.0
export CARGO_TARGET_DIR="$PWD/contracts/admin/target"
stellar contract build --manifest-path contracts/admin/Cargo.toml --locked
stellar contract build --manifest-path contracts/mainnet/timelock-controller/Cargo.toml --locked
