#!/usr/bin/env bash
set -euo pipefail

# Offline process-boundary regressions. No advisory fetches or live chain calls.
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TASK_TMP="$(mktemp -d)"
trap 'rm -rf -- "$TASK_TMP"' EXIT
export SECURITY_FIXTURE_ROOT="$ROOT" SECURITY_FIXTURE_LOG="$TASK_TMP/scanner.log"
export SECURITY_FIXTURE_MODE=clean

cargo() {
  local command_name="${1:-}" manifest="" previous="" argument
  for argument in "$@"; do
    if [[ "$previous" == "--manifest-path" ]]; then manifest="$argument"; fi
    previous="$argument"
  done
  case "$command_name" in
    locate-project)
      [[ " $* " == *" --workspace "* ]] || return 91
      [[ "$SECURITY_FIXTURE_MODE" != "workspace-failure" ]] || return 92
      if [[ "$SECURITY_FIXTURE_MODE" == "workspace-empty" ]]; then return 0; fi
      if [[ "$manifest" == "$SECURITY_FIXTURE_ROOT/contracts/mainnet-v2/mandate-registry/Cargo.toml" ]]; then
        printf '%s/Cargo.toml\n' "$SECURITY_FIXTURE_ROOT"
      else
        printf '%s\n' "$manifest"
      fi
      ;;
    tree)
      [[ " $* " == *" --locked "* && " $* " == *" --target wasm32v1-none "* ]] || return 93
      [[ " $* " == *" --edges normal "* && " $* " == *" --prefix none "* ]] || return 94
      case "$SECURITY_FIXTURE_MODE" in
        graph-failure) echo "Fixture dependency resolution failed" >&2; return 95 ;;
        graph-empty) return 0 ;;
        graph-partial-failure) printf 'mandate-registry v0.4.1\n'; return 96 ;;
        graph-paste) printf 'mandate-registry v0.4.1\npaste v1.0.15\n' ;;
        *) printf 'mandate-registry v0.4.1\nsoroban-sdk v26.1.0\n' ;;
      esac
      ;;
    *) echo "Unexpected Cargo call in offline security regression: $*" >&2; return 97 ;;
  esac
}

security_fixture_scanner() {
  [[ "$1" == "audit" && " $* " == *" --deny warnings "* ]] || return 98
  [[ " $* " == *" --ignore RUSTSEC-2024-0436 "* ]] || return 99
  [[ " $* " == *" --color never "* && " $* " == *" --format terminal "* ]] || return 89
  [[ "$PWD" == "$SECURITY_FIXTURE_ROOT" && -f .cargo/audit.toml ]] || return 88
  [[ "$SECURITY_FIXTURE_MODE" != "scanner-failure" ]] || return 90
  printf '%s\n' "$*" >>"$SECURITY_FIXTURE_LOG"
  printf '      Loaded 1239 security advisories\n    Updating crates.io index\n    Scanning Cargo.lock for vulnerabilities\n'
  # These are the pinned scanner's real operational messages. Upstream can
  # return success despite incomplete metadata; all such results must fail here.
  case "$SECURITY_FIXTURE_MODE" in
    scanner-index-update-zero) echo "warning: couldn't update crates.io index: fixture network failure" >&2 ;;
    scanner-index-open-zero) echo "warning: couldn't open crates.io index: fixture cache failure" >&2 ;;
    scanner-yank-zero) echo "error: couldn't check if the package is yanked: fixture missing metadata" >&2 ;;
    scanner-other-warning-zero) echo "warning: fixture unexpected operational warning" >&2 ;;
    scanner-other-error-zero) echo "error: fixture unexpected operational error" >&2 ;;
  esac
  return 0
}
export -f cargo security_fixture_scanner

run_scan() {
  CARGO_AUDIT=security_fixture_scanner bash "$ROOT/scripts/security-scan.sh"
}

run_scan >"$TASK_TMP/clean.log" 2>&1
[[ "$(wc -l <"$SECURITY_FIXTURE_LOG" | tr -d '[:space:]')" == "3" ]] || {
  echo "Security regression: all three build lockfiles must be scanned." >&2; exit 1;
}
grep -Fq -- "--file $ROOT/Cargo.lock --deny warnings --ignore RUSTSEC-2024-0436" "$SECURITY_FIXTURE_LOG" || {
  echo "Security regression: V2 must scan the actual root workspace lockfile." >&2; exit 1;
}
if grep -Fq -- "$ROOT/contracts/mainnet-v2/mandate-registry/Cargo.lock" "$SECURITY_FIXTURE_LOG"; then
  echo "Security regression: V2 scanned the inactive member lockfile." >&2; exit 1
fi

assert_rejected() {
  export SECURITY_FIXTURE_MODE="$1"
  local scan_status=0
  run_scan >"$TASK_TMP/$1.log" 2>&1 || scan_status=$?
  if [[ "$scan_status" == "0" ]]; then
    echo "Security regression: incomplete or unsafe result accepted: $1" >&2
    exit 1
  fi
  if [[ "$1" == "scanner-failure" && "$scan_status" != "90" ]]; then
    echo "Security regression: scanner failure status was not preserved." >&2
    exit 1
  fi
  if grep -Fq 'Security scan passed:' "$TASK_TMP/$1.log"; then
    echo "Security regression: failure printed a successful scan result: $1" >&2
    exit 1
  fi
}

for scenario in graph-paste graph-failure graph-partial-failure graph-empty workspace-failure workspace-empty scanner-failure scanner-index-update-zero scanner-index-open-zero scanner-yank-zero scanner-other-warning-zero scanner-other-error-zero; do
  assert_rejected "$scenario"
done

echo "Security scan offline regressions passed (13 scenarios): clean graphs, actual workspace locks, advisory rejection, resolution failures, empty/partial results, preserved scanner failure, and exit-zero operational errors."
