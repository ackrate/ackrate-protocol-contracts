#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RELEASE_DIR="$ROOT/target/mainnet-release"
REGISTRY_MANIFEST="$ROOT/contracts/mainnet/mandate-registry/Cargo.toml"
TIMELOCK_MANIFEST="$ROOT/contracts/mainnet/timelock-controller/Cargo.toml"

mkdir -p "$RELEASE_DIR"

for manifest in "$TIMELOCK_MANIFEST" "$REGISTRY_MANIFEST"; do
  cargo fmt --manifest-path "$manifest" --all -- --check
  cargo clippy --manifest-path "$manifest" --all-targets -- -D warnings
  cargo test --manifest-path "$manifest"
  stellar contract build \
    --locked \
    --manifest-path "$manifest" \
    --out-dir "$RELEASE_DIR"
done

stellar contract info interface \
  --wasm "$RELEASE_DIR/reapp_timelock_controller.wasm" \
  --output json-formatted > "$RELEASE_DIR/reapp_timelock_controller.interface.json"
stellar contract info interface \
  --wasm "$RELEASE_DIR/mandate_registry.wasm" \
  --output json-formatted > "$RELEASE_DIR/mandate_registry.interface.json"

(
  cd "$RELEASE_DIR"
  shasum -a 256 \
    reapp_timelock_controller.wasm \
    mandate_registry.wasm > SHA256SUMS
  wc -c \
    reapp_timelock_controller.wasm \
    mandate_registry.wasm > SIZES
)

echo "Mainnet candidate artifacts written to $RELEASE_DIR"
cat "$RELEASE_DIR/SHA256SUMS"
cat "$RELEASE_DIR/SIZES"
