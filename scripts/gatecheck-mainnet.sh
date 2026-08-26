#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RELEASE_DIR="$ROOT/target/mainnet-release"
REGISTRY_MANIFEST="$ROOT/contracts/mainnet/mandate-registry/Cargo.toml"
TIMELOCK_MANIFEST="$ROOT/contracts/mainnet/timelock-controller/Cargo.toml"

if [[ "$(uname -s)-$(uname -m)" != "Linux-x86_64" ]]; then
  echo "Canonical mainnet artifacts must be built on Ubuntu Linux x86_64." >&2
  echo "Use a successful main-branch GitHub run and ./scripts/fetch-mainnet-artifacts.sh on other platforms." >&2
  exit 4
fi

mkdir -p "$RELEASE_DIR"

if [[ "$(rustc --version)" != rustc\ 1.98.0\ * ]]; then
  echo "Mainnet release builds require Rust 1.98.0." >&2
  exit 2
fi
if [[ "$(stellar --version | head -n 1)" != stellar\ 27.0.0\ * ]]; then
  echo "Mainnet release builds require Stellar CLI 27.0.0." >&2
  exit 3
fi

for manifest in "$TIMELOCK_MANIFEST" "$REGISTRY_MANIFEST"; do
  cargo fmt --manifest-path "$manifest" --all -- --check
  cargo clippy --manifest-path "$manifest" --all-targets -- -D warnings
  cargo test --manifest-path "$manifest"
  stellar contract build \
    --locked \
    --optimize \
    --meta source_repo=github:ackrate/ackrate-protocol-contracts \
    --meta home_domain=ackrate.xyz \
    --manifest-path "$manifest" \
    --out-dir "$RELEASE_DIR"
done

stellar contract info interface \
  --wasm "$RELEASE_DIR/ackrate_timelock_controller.wasm" \
  --output json-formatted > "$RELEASE_DIR/ackrate_timelock_controller.interface.json"
stellar contract info interface \
  --wasm "$RELEASE_DIR/mandate_registry.wasm" \
  --output json-formatted > "$RELEASE_DIR/mandate_registry.interface.json"

(
  cd "$RELEASE_DIR"
  shasum -a 256 \
    ackrate_timelock_controller.wasm \
    mandate_registry.wasm > SHA256SUMS
  wc -c \
    ackrate_timelock_controller.wasm \
    mandate_registry.wasm > SIZES
)

node "$ROOT/scripts/check-mainnet-artifacts.mjs"

echo "Mainnet candidate artifacts written to $RELEASE_DIR"
cat "$RELEASE_DIR/SHA256SUMS"
cat "$RELEASE_DIR/SIZES"
