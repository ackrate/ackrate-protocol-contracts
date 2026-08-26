#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_AUDIT="${CARGO_AUDIT:-cargo-audit}"
ACCEPTED_HOST_ONLY_ADVISORY="RUSTSEC-2024-0436"

if ! command -v "$CARGO_AUDIT" >/dev/null 2>&1; then
  echo "cargo-audit is required: cargo install cargo-audit --version 0.22.2 --locked" >&2
  exit 1
fi

for contract in mandate-registry timelock-controller; do
  manifest="$ROOT/contracts/mainnet/$contract/Cargo.toml"
  lockfile="$ROOT/contracts/mainnet/$contract/Cargo.lock"

  echo "==> $contract: dependency vulnerabilities, yanked crates, and unexpected warnings"
  "$CARGO_AUDIT" audit \
    --file "$lockfile" \
    --deny warnings \
    --ignore "$ACCEPTED_HOST_ONLY_ADVISORY"

  echo "==> $contract: accepted advisory must not enter deployed WASM"
  if [[ -n "$(cargo tree --manifest-path "$manifest" --target wasm32v1-none -i paste 2>/dev/null)" ]]; then
    echo "paste entered the $contract wasm32v1-none dependency graph" >&2
    exit 1
  fi
done

echo "Security scan passed: zero vulnerabilities, zero yanked crates, no unexpected warnings, and no accepted advisory in deployed WASM graphs."
