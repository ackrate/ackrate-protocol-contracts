#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RELEASE_DIR="$ROOT/target/mainnet-release"
TIMELOCK_WASM="$RELEASE_DIR/ackrate_timelock_controller.wasm"
REGISTRY_WASM="$RELEASE_DIR/mandate_registry.wasm"
NETWORK_PASSPHRASE="Public Global Stellar Network ; September 2015"
MIN_TIMELOCK_DELAY_LEDGERS=17280

required=(
  ACKRATE_DEPLOYER
  ACKRATE_DEPLOYMENT_SOURCE_ACCOUNT
  ACKRATE_AUTHORITY_2_OF_3
  ACKRATE_AUTHORITY_MANIFEST
  ACKRATE_EMERGENCY_PAUSER
  ACKRATE_MAINNET_RPC_URL
  ACKRATE_MAINNET_USDC_SAC
  ACKRATE_TIMELOCK_DELAY_LEDGERS
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

if [[ "${ACKRATE_MAINNET_CONFIRM:-}" != "DEPLOY_EXACT_REVIEWED_BUILD" ]]; then
  echo "Submission guard is closed." >&2
  echo "Set ACKRATE_MAINNET_CONFIRM=DEPLOY_EXACT_REVIEWED_BUILD only after exact approval." >&2
  echo "No transaction was built or submitted." >&2
  exit 3
fi

if [[ ! -f "$TIMELOCK_WASM" || ! -f "$REGISTRY_WASM" ]]; then
  echo "Release artifacts are missing. Run ./scripts/gatecheck-mainnet.sh first." >&2
  exit 4
fi

if [[ "$(stellar --version | head -n 1)" != stellar\ 26.1.0\ * ]]; then
  echo "Mainnet deployment requires Stellar CLI 26.1.0." >&2
  exit 15
fi

if [[ ! "$ACKRATE_DEPLOYER" =~ ^[A-Za-z0-9._-]+$ ]]; then
  echo "ACKRATE_DEPLOYER must name a local Stellar CLI identity; do not pass a secret or seed phrase." >&2
  exit 13
fi

deployer_public_key="$(stellar keys address "$ACKRATE_DEPLOYER")"
if [[ "$deployer_public_key" != "$ACKRATE_DEPLOYMENT_SOURCE_ACCOUNT" ]]; then
  echo "ACKRATE_DEPLOYER does not resolve to ACKRATE_DEPLOYMENT_SOURCE_ACCOUNT." >&2
  exit 14
fi

node "$ROOT/scripts/check-mainnet-artifacts.mjs"
IFS=' ' read -r EXPECTED_TIMELOCK_HASH EXPECTED_REGISTRY_HASH \
  < <(node "$ROOT/scripts/check-mainnet-artifacts.mjs" --print-hashes)

export ACKRATE_MAINNET_NETWORK_PASSPHRASE="$NETWORK_PASSPHRASE"
node "$ROOT/scripts/preflight-mainnet.mjs"

rpc_args=(
  --rpc-url "$ACKRATE_MAINNET_RPC_URL"
  --network-passphrase "$NETWORK_PASSPHRASE"
)
if [[ -n "${ACKRATE_MAINNET_RPC_HEADER:-}" ]]; then
  rpc_args+=(--rpc-header "$ACKRATE_MAINNET_RPC_HEADER")
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

if [[ ! "$ACKRATE_AUTHORITY_2_OF_3" =~ ^G[A-Z2-7]{55}$ ]]; then
  echo "ACKRATE_AUTHORITY_2_OF_3 must be a public Stellar G-account." >&2
  exit 7
fi

if [[ ! "$ACKRATE_EMERGENCY_PAUSER" =~ ^G[A-Z2-7]{55}$ ]]; then
  echo "ACKRATE_EMERGENCY_PAUSER must be a public Stellar G-account." >&2
  exit 8
fi

if [[ ! "$ACKRATE_MAINNET_USDC_SAC" =~ ^C[A-Z2-7]{55}$ ]]; then
  echo "ACKRATE_MAINNET_USDC_SAC must be an independently verified contract ID." >&2
  exit 9
fi

if [[ ! "$ACKRATE_TIMELOCK_DELAY_LEDGERS" =~ ^[1-9][0-9]*$ ]] \
  || (( ACKRATE_TIMELOCK_DELAY_LEDGERS < MIN_TIMELOCK_DELAY_LEDGERS )) \
  || (( ACKRATE_TIMELOCK_DELAY_LEDGERS > 4294967295 )); then
  echo "ACKRATE_TIMELOCK_DELAY_LEDGERS must be at least $MIN_TIMELOCK_DELAY_LEDGERS and fit u32." >&2
  exit 10
fi

timelock_id="$(stellar contract deploy \
  "${rpc_args[@]}" \
  --source-account "$ACKRATE_DEPLOYER" \
  --wasm "$TIMELOCK_WASM" \
  --optimize=false \
  -- \
  --min-delay "$ACKRATE_TIMELOCK_DELAY_LEDGERS" \
  --proposers "[\"$ACKRATE_AUTHORITY_2_OF_3\"]" \
  --executors '[]' \
  --admin null)"

echo "Timelock deployed. Record this ID before continuing: $timelock_id" >&2

governance="{\"admin\":\"$timelock_id\",\"asset_policy\":\"$timelock_id\",\"pauser\":\"$ACKRATE_EMERGENCY_PAUSER\",\"unpauser\":\"$ACKRATE_AUTHORITY_2_OF_3\",\"upgrader\":\"$timelock_id\"}"

registry_id="$(stellar contract deploy \
  "${rpc_args[@]}" \
  --source-account "$ACKRATE_DEPLOYER" \
  --wasm "$REGISTRY_WASM" \
  --optimize=false \
  -- \
  --governance "$governance" \
  --initial-asset "$ACKRATE_MAINNET_USDC_SAC")"

observed_timelock_hash="$(stellar contract info hash \
  "${rpc_args[@]}" \
  --contract-id "$timelock_id")"
observed_registry_hash="$(stellar contract info hash \
  "${rpc_args[@]}" \
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
