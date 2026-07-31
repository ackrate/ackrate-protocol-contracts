#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RELEASE_DIR="$ROOT/target/mainnet-release"
TIMELOCK_WASM="$RELEASE_DIR/reapp_timelock_controller.wasm"
REGISTRY_WASM="$RELEASE_DIR/mandate_registry.wasm"
EXPECTED_TIMELOCK_HASH="766b79bab9208677ee151721f24a12b1c215a61728eafeaa540bd6d67df920b7"
EXPECTED_REGISTRY_HASH="9e16748606654c900d8b98655134fed0cdb2ebc5a0c314702ed1f030ef70b9d8"

required=(
  REAPP_DEPLOYER
  REAPP_AUTHORITY_2_OF_3
  REAPP_EMERGENCY_PAUSER
  REAPP_MAINNET_USDC_SAC
  REAPP_TIMELOCK_DELAY_LEDGERS
)

missing=()
for name in "${required[@]}"; do
  if [[ -z "${!name:-}" ]]; then
    missing+=("$name")
  fi
done

if (( ${#missing[@]} > 0 )); then
  echo "Missing public deployment inputs: ${missing[*]}" >&2
  echo "No transaction was built or submitted." >&2
  exit 2
fi

if [[ "${REAPP_MAINNET_CONFIRM:-}" != "DEPLOY_EXACT_REVIEWED_BUILD" ]]; then
  echo "Submission guard is closed." >&2
  echo "Set REAPP_MAINNET_CONFIRM=DEPLOY_EXACT_REVIEWED_BUILD only after exact approval." >&2
  echo "No transaction was built or submitted." >&2
  exit 3
fi

if [[ ! -f "$TIMELOCK_WASM" || ! -f "$REGISTRY_WASM" ]]; then
  echo "Release artifacts are missing. Run ./scripts/gatecheck-mainnet.sh first." >&2
  exit 4
fi

actual_timelock_hash="$(shasum -a 256 "$TIMELOCK_WASM" | awk '{print $1}')"
actual_registry_hash="$(shasum -a 256 "$REGISTRY_WASM" | awk '{print $1}')"

if [[ "$actual_timelock_hash" != "$EXPECTED_TIMELOCK_HASH" ]]; then
  echo "Timelock artifact hash does not match the reviewed candidate." >&2
  exit 5
fi

if [[ "$actual_registry_hash" != "$EXPECTED_REGISTRY_HASH" ]]; then
  echo "MandateRegistry artifact hash does not match the reviewed candidate." >&2
  exit 6
fi

if [[ ! "$REAPP_AUTHORITY_2_OF_3" =~ ^G[A-Z2-7]{55}$ ]]; then
  echo "REAPP_AUTHORITY_2_OF_3 must be a public Stellar G-account." >&2
  exit 7
fi

if [[ ! "$REAPP_EMERGENCY_PAUSER" =~ ^G[A-Z2-7]{55}$ ]]; then
  echo "REAPP_EMERGENCY_PAUSER must be a public Stellar G-account." >&2
  exit 8
fi

if [[ ! "$REAPP_MAINNET_USDC_SAC" =~ ^C[A-Z2-7]{55}$ ]]; then
  echo "REAPP_MAINNET_USDC_SAC must be an independently verified contract ID." >&2
  exit 9
fi

if [[ ! "$REAPP_TIMELOCK_DELAY_LEDGERS" =~ ^[1-9][0-9]*$ ]]; then
  echo "REAPP_TIMELOCK_DELAY_LEDGERS must be a positive ledger count." >&2
  exit 10
fi

timelock_id="$(stellar contract deploy \
  --network mainnet \
  --source-account "$REAPP_DEPLOYER" \
  --wasm "$TIMELOCK_WASM" \
  --optimize=false \
  -- \
  --min-delay "$REAPP_TIMELOCK_DELAY_LEDGERS" \
  --proposers "[\"$REAPP_AUTHORITY_2_OF_3\"]" \
  --executors '[]' \
  --admin null)"

echo "Timelock deployed. Record this ID before continuing: $timelock_id" >&2

governance="{\"admin\":\"$timelock_id\",\"asset_policy\":\"$timelock_id\",\"pauser\":\"$REAPP_EMERGENCY_PAUSER\",\"unpauser\":\"$REAPP_AUTHORITY_2_OF_3\",\"upgrader\":\"$timelock_id\"}"

registry_id="$(stellar contract deploy \
  --network mainnet \
  --source-account "$REAPP_DEPLOYER" \
  --wasm "$REGISTRY_WASM" \
  --optimize=false \
  -- \
  --governance "$governance" \
  --initial-asset "$REAPP_MAINNET_USDC_SAC")"

observed_timelock_hash="$(stellar contract info hash \
  --network mainnet \
  --contract-id "$timelock_id")"
observed_registry_hash="$(stellar contract info hash \
  --network mainnet \
  --contract-id "$registry_id")"

if [[ "$observed_timelock_hash" != "$EXPECTED_TIMELOCK_HASH" ]]; then
  echo "STOP: deployed timelock hash mismatch." >&2
  exit 11
fi

if [[ "$observed_registry_hash" != "$EXPECTED_REGISTRY_HASH" ]]; then
  echo "STOP: deployed MandateRegistry hash mismatch." >&2
  exit 12
fi

echo "Timelock contract ID: $timelock_id"
echo "MandateRegistry contract ID: $registry_id"
echo "Timelock WASM SHA-256: $observed_timelock_hash"
echo "MandateRegistry WASM SHA-256: $observed_registry_hash"
echo "Record the transaction hashes, constructor arguments, ledger, and independent checks in the private manifest."
