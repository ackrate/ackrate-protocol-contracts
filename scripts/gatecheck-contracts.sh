#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

forbidden_public_term="$(printf '\141\165\144\151\164')"
if grep -R -n -i --include='*.md' \
  "${forbidden_public_term}" \
  "$ROOT/README.md" "$ROOT/docs" "$ROOT/contracts"; then
  echo "Public documentation contains a prohibited security-review term." >&2
  exit 1
fi

echo "==> mainnet-v2 deployment workflow offline self-test"
bash "$ROOT/scripts/test-deploy-mainnet-v2.sh"

echo "==> unambiguous release tag routing"
bash "$ROOT/scripts/release-tag-route.sh" --self-test

stellar_version="$(stellar --version | head -n 1)"
if [[ "$stellar_version" != stellar\ 27.0.0* ]]; then
  echo "Stellar CLI 27.0.0 is required, found: $stellar_version" >&2
  exit 1
fi

echo "==> release workflow supply-chain pins"
bash "$ROOT/scripts/check-action-pins.sh"

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

for variant in simple mainnet-v2 composites; do
  contract="$ROOT/contracts/$variant/mandate-registry"
  echo "==> $variant: format"
  cargo fmt --manifest-path "$contract/Cargo.toml" --all -- --check
  echo "==> $variant: lint"
  cargo clippy --manifest-path "$contract/Cargo.toml" --locked --all-targets --all-features -- -D warnings
  if [[ "$variant" == "mainnet-v2" ]]; then
    echo "==> mainnet-v2: exact required test manifest"
    expected_tests="$(LC_ALL=C sort "$contract/tests.required")"
    actual_tests="$(
      cargo test \
        --manifest-path "$contract/Cargo.toml" \
        --locked \
        --all-features \
        -- --list 2>/dev/null |
        sed -n 's/: test$//p' |
        LC_ALL=C sort
    )"
    if [[ "$actual_tests" != "$expected_tests" ]]; then
      echo "mainnet-v2 required test manifest changed:" >&2
      diff -u <(printf '%s\n' "$expected_tests") <(printf '%s\n' "$actual_tests") >&2 || true
      exit 1
    fi
  fi
  echo "==> $variant: tests"
  cargo test --manifest-path "$contract/Cargo.toml" --locked -- --include-ignored
  echo "==> $variant: release WASM"
  if [[ "$variant" == "mainnet-v2" ]]; then
    stellar contract build \
      --manifest-path "$contract/Cargo.toml" \
      --package mandate-registry \
      --locked \
      --optimize \
      --meta source_repo=github:ackrate/ackrate-protocol-contracts \
      --meta home_domain=ackrate.xyz

    wasm="$contract/target/wasm32v1-none/release/mandate_registry.wasm"
    expected_wasm_size='15510'
    actual_wasm_size="$(wc -c <"$wasm" | tr -d '[:space:]')"
    if [[ "$actual_wasm_size" != "$expected_wasm_size" ]]; then
      echo "mainnet-v2 optimized WASM size changed: $actual_wasm_size" >&2
      exit 1
    fi
    expected_wasm_hash='982809197d35d44c7b0fce6bd117fb2fec09b728c64c146c1f803b01faacff62'
    actual_wasm_hash="$(shasum -a 256 "$wasm" | awk '{print $1}')"
    if [[ "${ACKRATE_CANONICAL_RELEASE_BUILD:-0}" == "1" ]]; then
      if [[ "$actual_wasm_hash" != "$expected_wasm_hash" ]]; then
        echo "mainnet-v2 canonical optimized WASM hash changed: $actual_wasm_hash" >&2
        exit 1
      fi
    else
      echo "==> mainnet-v2: canonical byte hash is enforced by the Linux release gate"
    fi
    interface="$(stellar contract info interface --wasm "$wasm" --output json)"
    expected_interface_hash='69c201ce1fb089ccfef06f125826b0aeba72af1b1536cb0b19e8cb05970ee805'
    actual_interface_hash="$(jq -S -c . <<<"$interface" | shasum -a 256 | awk '{print $1}')"
    if [[ "$actual_interface_hash" != "$expected_interface_hash" ]]; then
      echo "mainnet-v2 full interface schema changed: $actual_interface_hash" >&2
      exit 1
    fi
    expected_functions=$'__constructor\naccept_admin\nderive_mandate_id\nexecute_payment\nget_admin\nget_mandate\nget_pending_admin\nget_schema_version\nis_asset_allowed\nis_paused\npause\npropose_admin\nregister_mandate\nrevoke_mandate\nset_asset_allowed\nunpause\nupgrade\nvalidate_mandate'
    actual_functions="$(jq -r '.[].function_v0?.name // empty' <<<"$interface" | sort)"
    if [[ "$actual_functions" != "$expected_functions" ]]; then
      echo "mainnet-v2 exported function surface changed:" >&2
      echo "$actual_functions" >&2
      exit 1
    fi

    expected_events=$'AdminSet\nAdminTransferProposed\nAssetPolicyChanged\nMandateRegistered\nMandateRevoked\nPaused\nPaymentExecuted\nUnpaused\nUpgraded'
    actual_events="$(jq -r '.[].event_v0?.name // empty' <<<"$interface" | sort)"
    if [[ "$actual_events" != "$expected_events" ]]; then
      echo "mainnet-v2 exported event surface changed:" >&2
      echo "$actual_events" >&2
      exit 1
    fi

    echo "==> mainnet-v2: execute all-feature suite, including exact optimized-WASM smoke"
    MAINNET_V2_RELEASE_WASM="$wasm" cargo test \
      --manifest-path "$contract/Cargo.toml" \
      --locked \
      --all-features \
      -- --include-ignored
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
  cargo test --manifest-path "$manifest" --locked -- --include-ignored
done
