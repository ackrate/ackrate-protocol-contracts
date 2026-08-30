#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "==> release workflow supply-chain pins"
if grep -R -n -E --include='*.yml' --include='*.yaml' \
  'uses: [^#[:space:]]+@(main|master|v[0-9])' \
  "$ROOT/.github/workflows"; then
  echo "External GitHub Actions must be pinned to full commit SHAs." >&2
  exit 1
fi

if [[ -d "$ROOT/contracts/mainnet-v2/timelock-controller" ]]; then
  echo "mainnet-v2 must contain only mandate-registry; timelock-controller is forbidden." >&2
  exit 1
fi

if grep -R -n -E --include='*.rs' \
  'schedule_upgrade|cancel_upgrade|execute_upgrade|get_pending_upgrade|get_upgrade_delay|PendingUpgrade|UPGRADE_DELAY' \
  "$ROOT/contracts/mainnet-v2/mandate-registry/src"; then
  echo "mainnet-v2 must not expose timelock or delayed-upgrade state." >&2
  exit 1
fi

if grep -R -n -E -i --include='*.rs' \
  'mock_all_auths|mock_auths|MockAuth|dummy' \
  "$ROOT/contracts/mainnet-v2/mandate-registry/src"; then
  echo "mainnet-v2 must not use authorization bypasses or dummy data." >&2
  exit 1
fi

required_v2_negative_tests=(
  admin_methods_require_authorization
  execute_requires_agent_auth
  expired_mandate_rejected
  overspend_single_rejected
  overspend_cumulative_rejected
  replay_stale_seq_rejected
  upgrade_requires_pause_without_changing_state
)
for test_name in "${required_v2_negative_tests[@]}"; do
  if ! grep -R -q -E --include='*.rs' \
    "fn[[:space:]]+${test_name}[[:space:]]*\\(" \
    "$ROOT/contracts/mainnet-v2/mandate-registry/src"; then
    echo "mainnet-v2 continuous negative gate is missing: $test_name" >&2
    exit 1
  fi
done

for variant in simple mainnet-v2 composites; do
  contract="$ROOT/contracts/$variant/mandate-registry"
  echo "==> $variant: format"
  cargo fmt --manifest-path "$contract/Cargo.toml" --all -- --check
  echo "==> $variant: lint"
  cargo clippy --manifest-path "$contract/Cargo.toml" --locked --all-targets -- -D warnings
  echo "==> $variant: tests"
  cargo test --manifest-path "$contract/Cargo.toml" --locked
  echo "==> $variant: release WASM"
  if [[ "$variant" == "mainnet-v2" ]]; then
    stellar contract build \
      --manifest-path "$contract/Cargo.toml" \
      --package mandate-registry \
      --locked \
      --optimize

    wasm="$contract/target/wasm32v1-none/release/mandate_registry.wasm"
    interface="$(stellar contract info interface --wasm "$wasm" --output json)"
    expected_functions=$'__constructor\nexecute_payment\nget_admin\nget_mandate\nis_paused\npause\nregister_mandate\nrevoke_mandate\nset_admin\nunpause\nupgrade\nvalidate_mandate'
    actual_functions="$(jq -r '.[].function_v0?.name // empty' <<<"$interface" | sort)"
    if [[ "$actual_functions" != "$expected_functions" ]]; then
      echo "mainnet-v2 exported function surface changed:" >&2
      echo "$actual_functions" >&2
      exit 1
    fi

    expected_events=$'AdminSet\nMandateRegistered\nMandateRevoked\nPaused\nPaymentExecuted\nUnpaused\nUpgraded'
    actual_events="$(jq -r '.[].event_v0?.name // empty' <<<"$interface" | sort)"
    if [[ "$actual_events" != "$expected_events" ]]; then
      echo "mainnet-v2 exported event surface changed:" >&2
      echo "$actual_events" >&2
      exit 1
    fi
  else
    cargo build --manifest-path "$contract/Cargo.toml" --locked --target wasm32v1-none --release
  fi
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
