#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$ROOT/scripts/deploy-mainnet-v2.sh"
TMP_DIR="$(mktemp -d)"
CALL_LOG="$TMP_DIR/calls.log"
XDR_FILE="$TMP_DIR/reviewed.xdr"
UNSIGNED_XDR_FILE="$TMP_DIR/reviewed-unsigned.xdr"
SOURCE_G="GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN"
SIGNER_1_G="GADKNO6PXPSHPB5SEL5T7VZSBNIV7Z5WWHX5RGSZYNEUL3RKBEL2KVYK"
SIGNER_2_G="GCL6IG2J5QRO4B2XBXJ6QUGKEAMOCOS2HQIRMJWBPVEBZRDM5EZPR3TY"
SIGNER_3_G="GCQNLXZSQUYVFXYTWPA6RF6KIRTGQNZYHR4JIILXE3LTMXZQAUW6PXN5"
CANONICAL_USDC="CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75"
CONTRACT_C="CA3VSNZ2KOZLOKUXFMBFXOFHAQNHMK2PZ5NDRFXIACLCTDYYQ4USONNA"
TX_HASH="1111111111111111111111111111111111111111111111111111111111111111"
UNSIGNED_TX_HASH="$TX_HASH"
FIXTURE_ENVELOPE_KIND="tx"
FIXTURE_SIGNATURE_COUNT="2"
FIXTURE_DUPLICATE_SIGNATURES="0"
FIXTURE_UNSIGNED_SIGNATURE_COUNT="0"
FIXTURE_AUTHORITY_MODE="valid"
FIXTURE_DEPLOY_MODE="valid"

cleanup() {
  rm -rf -- "$TMP_DIR"
}
trap cleanup EXIT

printf 'AAAA\n' >"$XDR_FILE"
printf 'BBBB\n' >"$UNSIGNED_XDR_FILE"
: >"$CALL_LOG"
export CALL_LOG TX_HASH UNSIGNED_TX_HASH XDR_FILE UNSIGNED_XDR_FILE SOURCE_G SIGNER_1_G SIGNER_2_G SIGNER_3_G CANONICAL_USDC CONTRACT_C
export FIXTURE_ENVELOPE_KIND FIXTURE_SIGNATURE_COUNT FIXTURE_DUPLICATE_SIGNATURES FIXTURE_UNSIGNED_SIGNATURE_COUNT FIXTURE_AUTHORITY_MODE FIXTURE_DEPLOY_MODE

fixture_signatures() {
  case "$FIXTURE_SIGNATURE_COUNT" in
    0) printf '[]' ;;
    1) printf '[{"hint":"aaaa","signature":"sig-a"}]' ;;
    2)
      if [[ "$FIXTURE_DUPLICATE_SIGNATURES" == "1" ]]; then
        printf '[{"hint":"aaaa","signature":"sig-a"},{"hint":"bbbb","signature":"sig-a"}]'
      else
        printf '[{"hint":"aaaa","signature":"sig-a"},{"hint":"bbbb","signature":"sig-b"}]'
      fi
      ;;
    3) printf '[{"hint":"aaaa","signature":"sig-a"},{"hint":"bbbb","signature":"sig-b"},{"hint":"cccc","signature":"sig-c"}]' ;;
    *) return 89 ;;
  esac
}

stellar() {
  printf 'stellar %s\n' "$*" >>"$CALL_LOG"
  while [[ "${1:-}" == "--quiet" || "${1:-}" == "--no-cache" ]]; do
    shift
  done
  case "${1:-}" in
    --version)
      printf 'stellar 27.0.0 (offline-self-test)\n'
      ;;
    strkey)
      printf '00\n'
      ;;
    tx)
      case "${2:-}" in
        decode)
          local decoded_file="${*: -1}"
          local decoded_signatures
          if [[ "$decoded_file" == "$UNSIGNED_XDR_FILE" ]]; then
            if [[ "$FIXTURE_UNSIGNED_SIGNATURE_COUNT" == "0" ]]; then
              decoded_signatures='[]'
            else
              decoded_signatures='[{"hint":"aaaa","signature":"sig-a"}]'
            fi
          else
            decoded_signatures="$(fixture_signatures)"
          fi
          case "$FIXTURE_ENVELOPE_KIND" in
            tx) printf '{"tx":{"signatures":%s}}\n' "$decoded_signatures" ;;
            tx_v0) printf '{"tx_v0":{"signatures":%s}}\n' "$decoded_signatures" ;;
            fee_bump) printf '{"tx_fee_bump":{"signatures":%s}}\n' "$decoded_signatures" ;;
            *) printf '{"unexpected":true}\n' ;;
          esac
          ;;
        hash)
          if [[ "${*: -1}" == "$UNSIGNED_XDR_FILE" ]]; then
            printf '%s\n' "$UNSIGNED_TX_HASH"
          else
            printf '%s\n' "$TX_HASH"
          fi
          ;;
        new)
          [[ "${3:-}" == "set-options" && " $* " == *" --build-only "* ]] || return 97
          printf 'AAAA\n'
          ;;
        send)
          echo "self-test must never reach tx send" >&2
          return 90
          ;;
        *) return 91 ;;
      esac
      ;;
    contract)
      case "${2:-}" in
        info)
          [[ "${3:-}" == "hash" ]] || return 93
          printf 'acf5a71b86ad0d92f4f1249f827838e70a3bee5b5e56e6d2e50f047670037fc1\n'
          ;;
        invoke)
          local function_name=""
          while (( $# > 0 )); do
            if [[ "$1" == "--" ]]; then
              function_name="${2:-}"
              break
            fi
            shift
          done
          case "$function_name" in
            get_admin) printf '"%s"\n' "$SIGNER_1_G" ;;
            get_pending_admin) printf 'null\n' ;;
            get_schema_version) printf '2\n' ;;
            is_paused)
              if [[ "$FIXTURE_DEPLOY_MODE" == "paused" ]]; then
                printf 'true\n'
              else
                printf 'false\n'
              fi
              ;;
            is_asset_allowed) printf 'true\n' ;;
            *) return 94 ;;
          esac
          ;;
        *) return 95 ;;
      esac
      ;;
    *) return 92 ;;
  esac
}
export -f stellar

curl() {
  printf 'curl %s\n' "$*" >>"$CALL_LOG"
  local url="${*: -1}"
  case "$url" in
    */accounts/"$SIGNER_1_G")
      if [[ "$FIXTURE_AUTHORITY_MODE" == "bad_weight" ]]; then
        printf '{"account_id":"%s","signers":[{"key":"%s","type":"ed25519_public_key","weight":2},{"key":"%s","type":"ed25519_public_key","weight":1},{"key":"%s","type":"ed25519_public_key","weight":1}],"thresholds":{"low_threshold":2,"med_threshold":2,"high_threshold":2},"balances":[{"asset_type":"native","balance":"10.0000000"}]}\n' \
          "$SIGNER_1_G" "$SIGNER_1_G" "$SIGNER_2_G" "$SIGNER_3_G"
      else
        printf '{"account_id":"%s","signers":[{"key":"%s","type":"ed25519_public_key","weight":1},{"key":"%s","type":"ed25519_public_key","weight":1},{"key":"%s","type":"ed25519_public_key","weight":1}],"thresholds":{"low_threshold":2,"med_threshold":2,"high_threshold":2},"balances":[{"asset_type":"native","balance":"10.0000000"}]}\n' \
          "$SIGNER_1_G" "$SIGNER_1_G" "$SIGNER_2_G" "$SIGNER_3_G"
      fi
      ;;
    */accounts/"$SOURCE_G")
      printf '{"account_id":"%s","balances":[{"asset_type":"native","balance":"10.0000000"}]}\n' "$SOURCE_G"
      ;;
    */)
      printf '{"network_passphrase":"Public Global Stellar Network ; September 2015"}\n'
      ;;
    *) return 96 ;;
  esac
}
export -f curl fixture_signatures

assert_fails_with() {
  local expected="$1"
  shift
  local output
  if output="$($SCRIPT "$@" 2>&1)"; then
    echo "Expected failure, but command succeeded: $*" >&2
    exit 1
  fi
  [[ "$output" == *"$expected"* ]] || {
    echo "Failure did not contain expected text: $expected" >&2
    echo "$output" >&2
    exit 1
  }
}

assert_no_send() {
  if grep -Eq '(^| )tx send( |$)' "$CALL_LOG"; then
    echo "Offline self-test detected a broadcast attempt." >&2
    cat "$CALL_LOG" >&2
    exit 1
  fi
}

"$SCRIPT" help >/dev/null
assert_no_send

"$SCRIPT" prepare-add-signer \
  --source "$SIGNER_1_G" \
  --signer "$SIGNER_2_G" \
  --rpc-url https://rpc.example.org \
  --out "$TMP_DIR/add-alex.xdr" >/dev/null
[[ "$(<"$TMP_DIR/add-alex.xdr")" == "AAAA" ]] \
  || { echo "Unsigned add-signer fixture XDR was not written." >&2; exit 1; }

"$SCRIPT" prepare-thresholds \
  --source "$SIGNER_1_G" \
  --rpc-url https://rpc.example.org \
  --out "$TMP_DIR/thresholds.xdr" >/dev/null
[[ "$(<"$TMP_DIR/thresholds.xdr")" == "AAAA" ]] \
  || { echo "Unsigned thresholds fixture XDR was not written." >&2; exit 1; }

grep -Eq 'tx new set-options .*--signer .*--signer-weight 1 .*--build-only' "$CALL_LOG" \
  || { echo "Add-signer preparation lost exact weight 1 or build-only." >&2; exit 1; }
grep -Eq 'tx new set-options .*--master-weight 1 .*--low-threshold 2 .*--med-threshold 2 .*--high-threshold 2 .*--build-only' "$CALL_LOG" \
  || { echo "Threshold preparation lost master 1, thresholds 2/2/2, or build-only." >&2; exit 1; }
assert_no_send

assert_fails_with "public Stellar G-account" \
  prepare-upload \
  --source "sample seed phrase must never work" \
  --rpc-url https://rpc.example.org \
  --out "$TMP_DIR/upload.xdr"

assert_fails_with "public Stellar G-account" \
  prepare-upload \
  --source "SAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" \
  --rpc-url https://rpc.example.org \
  --out "$TMP_DIR/upload.xdr"

assert_fails_with "public Stellar C-contract" \
  prepare-deploy \
  --source "$SIGNER_1_G" \
  --admin "$SIGNER_1_G" \
  --initial-asset "$SOURCE_G" \
  --observed-wasm-hash acf5a71b86ad0d92f4f1249f827838e70a3bee5b5e56e6d2e50f047670037fc1 \
  --rpc-url https://rpc.example.org \
  --out "$TMP_DIR/deploy.xdr"

assert_fails_with "does not equal the reviewed V2 hash" \
  prepare-deploy \
  --source "$SIGNER_1_G" \
  --admin "$SIGNER_1_G" \
  --initial-asset "$CANONICAL_USDC" \
  --observed-wasm-hash aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa \
  --rpc-url https://rpc.example.org \
  --out "$TMP_DIR/deploy.xdr"

assert_fails_with "canonical Circle Stellar Mainnet USDC SAC" \
  prepare-deploy \
  --source "$SIGNER_1_G" \
  --admin "$SIGNER_1_G" \
  --initial-asset "$CONTRACT_C" \
  --observed-wasm-hash acf5a71b86ad0d92f4f1249f827838e70a3bee5b5e56e6d2e50f047670037fc1 \
  --rpc-url https://rpc.example.org \
  --out "$TMP_DIR/deploy.xdr"

assert_fails_with "--source and --admin must be the same" \
  prepare-deploy \
  --source "$SOURCE_G" \
  --admin "$SIGNER_1_G" \
  --initial-asset "$CANONICAL_USDC" \
  --observed-wasm-hash acf5a71b86ad0d92f4f1249f827838e70a3bee5b5e56e6d2e50f047670037fc1 \
  --rpc-url https://rpc.example.org \
  --out "$TMP_DIR/deploy.xdr"

"$SCRIPT" preflight-authority \
  --authority "$SIGNER_1_G" \
  --source "$SIGNER_1_G" \
  --signer-1 "$SIGNER_1_G" \
  --signer-2 "$SIGNER_2_G" \
  --signer-3 "$SIGNER_3_G" \
  --horizon-url https://horizon.example.org >/dev/null

assert_fails_with "--source and --authority must be the same" \
  preflight-authority \
  --authority "$SIGNER_1_G" \
  --source "$SOURCE_G" \
  --signer-1 "$SIGNER_1_G" \
  --signer-2 "$SIGNER_2_G" \
  --signer-3 "$SIGNER_3_G" \
  --horizon-url https://horizon.example.org

assert_fails_with "--authority and --signer-1 must be the same" \
  preflight-authority \
  --authority "$SOURCE_G" \
  --source "$SOURCE_G" \
  --signer-1 "$SIGNER_1_G" \
  --signer-2 "$SIGNER_2_G" \
  --signer-3 "$SIGNER_3_G" \
  --horizon-url https://horizon.example.org

FIXTURE_AUTHORITY_MODE="bad_weight"
assert_fails_with "weight-1 Ed25519 signers with thresholds 2/2/2" \
  preflight-authority \
  --authority "$SIGNER_1_G" \
  --source "$SIGNER_1_G" \
  --signer-1 "$SIGNER_1_G" \
  --signer-2 "$SIGNER_2_G" \
  --signer-3 "$SIGNER_3_G" \
  --horizon-url https://horizon.example.org
FIXTURE_AUTHORITY_MODE="valid"

assert_fails_with "credential-free HTTPS URL" \
  preflight-authority \
  --authority "$SIGNER_1_G" \
  --source "$SIGNER_1_G" \
  --signer-1 "$SIGNER_1_G" \
  --signer-2 "$SIGNER_2_G" \
  --signer-3 "$SIGNER_3_G" \
  --horizon-url 'https://user:password@horizon.example.org'

assert_fails_with "credential-free HTTPS URL" \
  submit \
  --xdr "$XDR_FILE" \
  --reviewed-unsigned "$UNSIGNED_XDR_FILE" \
  --rpc-url 'https://user:password@rpc.example.org'

assert_fails_with "requires --xdr, --reviewed-unsigned, and --rpc-url" \
  submit \
  --xdr "$XDR_FILE" \
  --rpc-url https://rpc.example.org

assert_fails_with "Submission guard is CLOSED" \
  submit \
  --xdr "$XDR_FILE" \
  --reviewed-unsigned "$UNSIGNED_XDR_FILE" \
  --rpc-url https://rpc.example.org

assert_fails_with "Submission guard is CLOSED" \
  submit \
  --xdr "$XDR_FILE" \
  --reviewed-unsigned "$UNSIGNED_XDR_FILE" \
  --rpc-url https://rpc.example.org \
  --confirm BROADCAST_MAINNET_WRONG_FILE

for signature_count in 0 1 3; do
  FIXTURE_SIGNATURE_COUNT="$signature_count"
  assert_fails_with "exactly 2 distinct envelope signatures" \
    submit --xdr "$XDR_FILE" --reviewed-unsigned "$UNSIGNED_XDR_FILE" --rpc-url https://rpc.example.org
done
FIXTURE_SIGNATURE_COUNT="2"

FIXTURE_DUPLICATE_SIGNATURES="1"
assert_fails_with "exactly 2 distinct envelope signatures" \
  submit --xdr "$XDR_FILE" --reviewed-unsigned "$UNSIGNED_XDR_FILE" --rpc-url https://rpc.example.org
FIXTURE_DUPLICATE_SIGNATURES="0"

UNSIGNED_TX_HASH="2222222222222222222222222222222222222222222222222222222222222222"
assert_fails_with "does not match the exact reviewed unsigned transaction" \
  submit --xdr "$XDR_FILE" --reviewed-unsigned "$UNSIGNED_XDR_FILE" --rpc-url https://rpc.example.org
UNSIGNED_TX_HASH="$TX_HASH"

FIXTURE_UNSIGNED_SIGNATURE_COUNT="1"
assert_fails_with "reviewed unsigned XDR must contain zero signatures" \
  submit --xdr "$XDR_FILE" --reviewed-unsigned "$UNSIGNED_XDR_FILE" --rpc-url https://rpc.example.org
FIXTURE_UNSIGNED_SIGNATURE_COUNT="0"

FIXTURE_ENVELOPE_KIND="fee_bump"
assert_fails_with "Fee-bump envelopes are not supported" \
  submit --xdr "$XDR_FILE" --reviewed-unsigned "$UNSIGNED_XDR_FILE" --rpc-url https://rpc.example.org
FIXTURE_ENVELOPE_KIND="tx_v0"
assert_fails_with "Submission guard is CLOSED" \
  submit --xdr "$XDR_FILE" --reviewed-unsigned "$UNSIGNED_XDR_FILE" --rpc-url https://rpc.example.org
FIXTURE_ENVELOPE_KIND="tx"

"$SCRIPT" verify-deploy \
  --contract-id "$CONTRACT_C" \
  --source "$SOURCE_G" \
  --admin "$SIGNER_1_G" \
  --initial-asset "$CANONICAL_USDC" \
  --rpc-url https://rpc.example.org >/dev/null

FIXTURE_DEPLOY_MODE="paused"
assert_fails_with "not initially unpaused" \
  verify-deploy \
  --contract-id "$CONTRACT_C" \
  --source "$SOURCE_G" \
  --admin "$SIGNER_1_G" \
  --initial-asset "$CANONICAL_USDC" \
  --rpc-url https://rpc.example.org
FIXTURE_DEPLOY_MODE="valid"

hidden_input_output=""
if hidden_input_output="$(STELLAR_ACCOUNT="$SOURCE_G" "$SCRIPT" inspect-xdr --xdr "$XDR_FILE" 2>&1)"; then
  echo "Hidden Stellar signing/account environment input was accepted." >&2
  exit 1
fi
[[ "$hidden_input_output" == *"must be unset"* ]] || {
  echo "Hidden-input failure did not explain how to close the guard." >&2
  echo "$hidden_input_output" >&2
  exit 1
}

assert_no_send

invoke_count="$(grep -Ec 'stellar .*contract invoke' "$CALL_LOG")"
read_only_invoke_count="$(grep -Ec 'stellar .*contract invoke .*--send=no' "$CALL_LOG")"
if [[ "$invoke_count" == "0" || "$invoke_count" != "$read_only_invoke_count" ]]; then
  echo "Every post-deploy contract verification must remain read-only." >&2
  cat "$CALL_LOG" >&2
  exit 1
fi

if grep -Eq -- '--sign-with-key|--auto-sign|keys generate|keys add' "$SCRIPT"; then
  echo "Deployment workflow contains a forbidden local signing or key-caching path." >&2
  exit 1
fi

build_only_count="$(grep -Ec -- '^[[:space:]]+--build-only' "$SCRIPT")"
if [[ "$build_only_count" != "4" ]]; then
  echo "All four preparation stages must retain their unsigned build-only boundary." >&2
  exit 1
fi

if grep -Eq -- 'stellar .*tx sign|stellar .*contract (upload|deploy).*(--sign-with|--auto-sign)' "$SCRIPT"; then
  echo "Deployment workflow contains an automated signing path." >&2
  exit 1
fi

echo "Mainnet V2 deployment workflow offline self-test passed."
echo "  secret/phrase/address validation: fail closed"
echo "  signer/threshold setup: exact unsigned Set Options XDR"
echo "  wrong observed WASM hash: fail closed"
echo "  exact Circle USDC SAC: enforced"
echo "  authority signer set/weights/thresholds/source balance: fail closed"
echo "  envelope signatures: exactly 2 distinct; fee bumps rejected"
echo "  signed transaction matches exact zero-signature reviewed XDR: enforced"
echo "  deployed hash/admin/pending/schema/pause/asset reads: fail closed and read-only"
echo "  credential-bearing RPC URL: fail closed"
echo "  missing/wrong broadcast confirmation: fail closed"
echo "  default and preparation paths: no tx send"
