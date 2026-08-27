#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE_ROOT="${ACKRATE_MAINNET_SOURCE_ROOT:-$ROOT}"
RELEASE_DIR="$ROOT/target/mainnet-release"
REGISTRY_MANIFEST="$SOURCE_ROOT/contracts/mainnet/mandate-registry/Cargo.toml"
TIMELOCK_MANIFEST="$SOURCE_ROOT/contracts/mainnet/timelock-controller/Cargo.toml"
EXPECTED_SOURCE_COMMIT="$(node -p "JSON.parse(require('node:fs').readFileSync('$ROOT/contracts/mainnet/deployment-manifest.template.json', 'utf8')).source.commit")"

if [[ "$(uname -s)-$(uname -m)" != "Linux-x86_64" ]]; then
  echo "Canonical mainnet artifacts must be built on Ubuntu Linux x86_64." >&2
  echo "Use a successful main-branch GitHub run and ./scripts/fetch-mainnet-artifacts.sh on other platforms." >&2
  exit 4
fi

mkdir -p "$RELEASE_DIR"

if [[ "$(git -C "$SOURCE_ROOT" rev-parse HEAD)" != "$EXPECTED_SOURCE_COMMIT" ]]; then
  echo "Mainnet release source must be the exact reviewed commit $EXPECTED_SOURCE_COMMIT." >&2
  exit 5
fi
if [[ -n "$(git -C "$SOURCE_ROOT" status --porcelain)" ]]; then
  echo "Mainnet release source checkout must be clean." >&2
  exit 6
fi
if [[ "$(cd "$SOURCE_ROOT" && rustc --version)" != rustc\ 1.96.0\ * ]]; then
  echo "The reviewed canary source build requires Rust 1.96.0." >&2
  exit 2
fi
if [[ "$(stellar --version | head -n 1)" != stellar\ 27.0.0\ * ]]; then
  echo "Mainnet release builds require Stellar CLI 27.0.0." >&2
  exit 3
fi

for manifest in "$TIMELOCK_MANIFEST" "$REGISTRY_MANIFEST"; do
  (
    cd "$SOURCE_ROOT"
    cargo fmt --manifest-path "$manifest" --all -- --check
    cargo clippy --manifest-path "$manifest" --locked --all-targets -- -D warnings
    cargo test --manifest-path "$manifest" --locked
  )
done

# Reproduce the official StellarExpert release builder exactly: build from each
# package directory, select the package explicitly, and let Cargo use the
# checked-in lockfile by default. Changing the working directory or adding
# manifest flags changes the optimized WASM bytes even with the same sources.
(
  cd "$(dirname "$TIMELOCK_MANIFEST")"
  stellar contract build \
    --locked \
    --optimize \
    --package ackrate-timelock-controller \
    --out-dir "$RELEASE_DIR" \
    --meta source_repo=github:ackrate/ackrate-protocol-contracts \
    --meta home_domain=ackrate.xyz
)
(
  cd "$(dirname "$REGISTRY_MANIFEST")"
  stellar contract build \
    --locked \
    --optimize \
    --package mandate-registry \
    --out-dir "$RELEASE_DIR" \
    --meta source_repo=github:ackrate/ackrate-protocol-contracts \
    --meta home_domain=ackrate.xyz
)

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
