#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "==> release workflow supply-chain pins"
if rg --line-number 'uses: [^#[:space:]]+@(main|master|v[0-9])' "$ROOT/.github/workflows"; then
  echo "External GitHub Actions must be pinned to full commit SHAs." >&2
  exit 1
fi

for variant in simple composites; do
  contract="$ROOT/contracts/$variant/mandate-registry"
  echo "==> $variant: format"
  cargo fmt --manifest-path "$contract/Cargo.toml" --all -- --check
  echo "==> $variant: lint"
  cargo clippy --manifest-path "$contract/Cargo.toml" --locked --all-targets -- -D warnings
  echo "==> $variant: tests"
  cargo test --manifest-path "$contract/Cargo.toml" --locked
  echo "==> $variant: release WASM"
  cargo build --manifest-path "$contract/Cargo.toml" --locked --target wasm32v1-none --release
done

for contract in mandate-registry timelock-controller; do
  manifest="$ROOT/contracts/mainnet/$contract/Cargo.toml"
  echo "==> mainnet/$contract: format"
  cargo fmt --manifest-path "$manifest" --all -- --check
  echo "==> mainnet/$contract: lint"
  cargo clippy --manifest-path "$manifest" --locked --all-targets -- -D warnings
  echo "==> mainnet/$contract: tests"
  cargo test --manifest-path "$manifest" --locked
done
