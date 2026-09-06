#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_AUDIT="${CARGO_AUDIT:-cargo-audit}"
ACCEPTED_HOST_ONLY_ADVISORY="RUSTSEC-2024-0436"

[[ -f "$ROOT/.cargo/audit.toml" ]] || {
  echo "The repository security scanner configuration is missing." >&2
  exit 1
}

if ! command -v "$CARGO_AUDIT" >/dev/null 2>&1; then
  echo "cargo-audit is required: cargo install cargo-audit --version 0.22.2 --locked" >&2
  exit 1
fi

contracts=(
  "mainnet/mandate-registry"
  "mainnet/timelock-controller"
  "mainnet-v2/mandate-registry"
)

for contract in "${contracts[@]}"; do
  manifest="$ROOT/contracts/$contract/Cargo.toml"
  # Cargo resolves workspace members against the workspace lockfile, not a
  # possibly stale copy beside the member manifest. Scan exactly that input.
  if ! workspace_manifest="$(cargo locate-project --workspace --manifest-path "$manifest" --message-format plain)"; then
    echo "Cannot resolve the $contract workspace; security scan is incomplete." >&2
    exit 1
  fi
  [[ -n "$workspace_manifest" && -f "$workspace_manifest" ]] || {
    echo "Cargo returned no valid workspace manifest for $contract." >&2
    exit 1
  }
  lockfile="$(dirname "$workspace_manifest")/Cargo.lock"
  [[ -f "$lockfile" ]] || {
    echo "The resolved $contract build lockfile is missing: $lockfile" >&2
    exit 1
  }

  echo "==> $contract: dependency vulnerabilities, yanked crates, and unexpected warnings"
  # The pinned scanner can print an index/yank lookup error and still exit zero.
  # Keep terminal diagnostics visible, preserve failures, and reject every
  # operational warning/error, including recoverable lock warnings. A clean
  # rerun is required; incomplete metadata never qualifies as a passing scan.
  scanner_status=0
  scanner_output="$(cd "$ROOT" && "$CARGO_AUDIT" audit \
    --file "$lockfile" \
    --deny warnings \
    --ignore "$ACCEPTED_HOST_ONLY_ADVISORY" \
    --color never \
    --format terminal 2>&1)" || scanner_status=$?
  printf '%s\n' "$scanner_output"
  if [[ "$scanner_status" -ne 0 ]]; then
    exit "$scanner_status"
  fi
  if grep -Eiq '^[[:space:]]*(warning|error):|couldn.t (update crates.io index|open crates.io index|check if the package is yanked)' <<<"$scanner_output"; then
    echo "The $contract dependency scanner reported an operational warning/error; security scan is incomplete." >&2
    exit 1
  fi

  echo "==> $contract: accepted advisory must not enter deployed WASM"
  # An inverse lookup can fail both when paste is absent and when resolution
  # breaks. Resolve the complete deployed dependency graph successfully first;
  # never interpret a failed or empty graph as proof that an advisory is absent.
  if ! wasm_dependencies="$(cargo tree --manifest-path "$manifest" --locked --target wasm32v1-none --edges normal --prefix none --format '{p}')"; then
    echo "Cannot resolve the $contract WASM graph; security scan is incomplete." >&2
    exit 1
  fi
  [[ -n "$wasm_dependencies" ]] || {
    echo "Cargo returned an empty $contract WASM graph; security scan is incomplete." >&2
    exit 1
  }
  if grep -Eq '^paste v[0-9]' <<<"$wasm_dependencies"; then
    echo "paste entered the $contract wasm32v1-none dependency graph" >&2
    exit 1
  fi
done

echo "Security scan passed: zero vulnerabilities, zero yanked crates, no unexpected warnings, and no accepted advisory in deployed WASM graphs."
