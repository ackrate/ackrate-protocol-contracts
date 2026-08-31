#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONTRACT_DIR="$ROOT/contracts/mainnet-v2/mandate-registry"
DEFAULT_WASM="$CONTRACT_DIR/target/wasm32v1-none/release/mandate_registry.wasm"
NETWORK_PASSPHRASE="Public Global Stellar Network ; September 2015"
LAB_SIGN_URL="https://lab.stellar.org/transaction/import"
CANONICAL_USDC_SAC="CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75"

EXPECTED_WASM_SIZE="15510"
EXPECTED_WASM_HASH="982809197d35d44c7b0fce6bd117fb2fec09b728c64c146c1f803b01faacff62"
EXPECTED_INTERFACE_HASH="69c201ce1fb089ccfef06f125826b0aeba72af1b1536cb0b19e8cb05970ee805"
EXPECTED_FUNCTIONS=$'__constructor\naccept_admin\nderive_mandate_id\nexecute_payment\nget_admin\nget_mandate\nget_pending_admin\nget_schema_version\nis_asset_allowed\nis_paused\npause\npropose_admin\nregister_mandate\nrevoke_mandate\nset_asset_allowed\nunpause\nupgrade\nvalidate_mandate'
EXPECTED_EVENTS=$'AdminSet\nAdminTransferProposed\nAssetPolicyChanged\nMandateRegistered\nMandateRevoked\nPaused\nPaymentExecuted\nUnpaused\nUpgraded'

die() {
  echo "ERROR: $*" >&2
  exit 1
}

usage() {
  cat <<'EOF'
ACKRATE Mainnet V2 deployment preparation (unsigned by default)

Usage:
  ./scripts/deploy-mainnet-v2.sh build
  ./scripts/deploy-mainnet-v2.sh verify-wasm [--wasm FILE]
  ./scripts/deploy-mainnet-v2.sh prepare-add-signer --source G... --signer G... --rpc-url URL --out FILE.xdr
  ./scripts/deploy-mainnet-v2.sh prepare-thresholds --source G... --rpc-url URL --out FILE.xdr
  ./scripts/deploy-mainnet-v2.sh prepare-upload --source G... --rpc-url URL --out FILE.xdr [--wasm FILE]
  ./scripts/deploy-mainnet-v2.sh preflight-authority --authority G... --source G... --signer-1 G... --signer-2 G... --signer-3 G... --horizon-url URL
  ./scripts/deploy-mainnet-v2.sh inspect-xdr --xdr FILE.xdr
  ./scripts/deploy-mainnet-v2.sh lab-signing-guide --xdr FILE.xdr
  ./scripts/deploy-mainnet-v2.sh submit --xdr SIGNED.xdr --reviewed-unsigned UNSIGNED.xdr --rpc-url URL --confirm BROADCAST_MAINNET_<TX_HASH>
  ./scripts/deploy-mainnet-v2.sh prepare-deploy --source G... --admin G... --initial-asset C... --observed-wasm-hash HASH --rpc-url URL --out FILE.xdr [--wasm FILE]
  ./scripts/deploy-mainnet-v2.sh verify-deploy --contract-id C... --source G... --admin G... --initial-asset C... --rpc-url URL

The script accepts public Stellar G/C addresses only. It never accepts an
identity alias, secret key, seed phrase, or signing key. Preparation commands
only create unsigned XDR. Signing happens in Stellar Laboratory with Freighter.
Broadcast is a separate command and remains blocked unless --confirm contains
the exact hash of the XDR being submitted.
EOF
}

require_value() {
  local flag="$1"
  local value="${2:-}"
  [[ -n "$value" && "$value" != --* ]] || die "$flag requires a value."
}

reject_hidden_signing_inputs() {
  local name
  for name in \
    STELLAR_ACCOUNT \
    STELLAR_SIGN_WITH_KEY \
    STELLAR_SIGN_WITH_LAB \
    STELLAR_SIGN_WITH_LEDGER \
    STELLAR_AUTO_SIGN; do
    if [[ -n "${!name:-}" ]]; then
      die "$name must be unset. This workflow does not accept hidden signing inputs."
    fi
  done
}

require_stellar_cli() {
  command -v stellar >/dev/null 2>&1 || die "Stellar CLI 27.0.0 is required."
  local version
  version="$(stellar --version)"
  [[ "$version" == stellar\ 27.0.0* ]] \
    || die "Stellar CLI 27.0.0 is required; found: ${version%%$'\n'*}"
}

assert_g_address_shape() {
  local label="$1"
  local value="$2"
  [[ "$value" =~ ^G[A-Z2-7]{55}$ ]] \
    || die "$label must be one public Stellar G-account. Secret keys, seed phrases, and aliases are rejected."
}

assert_c_address_shape() {
  local label="$1"
  local value="$2"
  [[ "$value" =~ ^C[A-Z2-7]{55}$ ]] \
    || die "$label must be one public Stellar C-contract address. Secret keys, seed phrases, and aliases are rejected."
}

assert_valid_strkey() {
  local label="$1"
  local value="$2"
  stellar strkey decode "$value" >/dev/null 2>&1 \
    || die "$label has an invalid Stellar checksum."
}

assert_rpc_url() {
  local value="$1"
  [[ "$value" =~ ^https://[A-Za-z0-9][A-Za-z0-9.-]*(:[0-9]{1,5})?(/[^[:space:]?#]*)?$ ]] \
    && [[ "$value" != *"@"* ]] \
    || die "--rpc-url must be a credential-free HTTPS URL without a query or fragment."
}

assert_horizon_url() {
  local value="$1"
  [[ "$value" =~ ^https://[A-Za-z0-9][A-Za-z0-9.-]*(:[0-9]{1,5})?(/[^[:space:]?#]*)?$ ]] \
    && [[ "$value" != *"@"* ]] \
    || die "--horizon-url must be a credential-free HTTPS URL without a query or fragment."
}

assert_hash() {
  local label="$1"
  local value="$2"
  [[ "$value" =~ ^[0-9a-f]{64}$ ]] || die "$label must be one lowercase SHA-256 hash."
}

assert_new_xdr_path() {
  local path="$1"
  [[ "$path" == *.xdr ]] || die "--out must end in .xdr."
  [[ ! -e "$path" && ! -L "$path" ]] || die "Refusing to overwrite existing output: $path"
  [[ -d "$(dirname "$path")" ]] || die "Output directory does not exist: $(dirname "$path")"
}

assert_xdr_file() {
  local path="$1"
  [[ -f "$path" && ! -L "$path" ]] || die "XDR input must be a regular, non-symlink file: $path"
  local size
  size="$(wc -c <"$path" | tr -d '[:space:]')"
  [[ "$size" =~ ^[1-9][0-9]*$ ]] && (( size <= 10485760 )) \
    || die "XDR input must be non-empty and at most 10 MiB."
  awk '
    NR != 1 { exit 1 }
    $0 !~ /^[A-Za-z0-9+\/=]+$/ { exit 1 }
    END { if (NR != 1) exit 1 }
  ' "$path" || die "XDR input must contain exactly one base64 line."
  local xdr
  xdr="$(<"$path")"
  (( ${#xdr} % 4 == 0 )) || die "XDR input is not padded base64."
}

write_new_xdr() {
  local path="$1"
  local xdr="$2"
  [[ "$xdr" =~ ^[A-Za-z0-9+/]+=*$ ]] && (( ${#xdr} % 4 == 0 )) \
    || die "Stellar CLI did not return one base64 transaction envelope. Nothing was written."
  umask 077
  if ! (set -o noclobber; printf '%s\n' "$xdr" >"$path"); then
    die "Could not create new XDR file: $path"
  fi
  echo "Unsigned XDR created: $path"
  echo "Nothing was signed or broadcast."
}

verify_wasm() {
  local wasm="$1"
  [[ -f "$wasm" && ! -L "$wasm" ]] \
    || die "Reviewed V2 WASM is missing or is a symlink: $wasm"
  command -v jq >/dev/null 2>&1 || die "jq is required to verify the contract interface."
  command -v shasum >/dev/null 2>&1 || die "shasum is required to verify the contract."

  local actual_size actual_hash interface actual_interface_hash actual_functions actual_events
  actual_size="$(wc -c <"$wasm" | tr -d '[:space:]')"
  [[ "$actual_size" == "$EXPECTED_WASM_SIZE" ]] \
    || die "V2 WASM size mismatch: expected $EXPECTED_WASM_SIZE bytes, got $actual_size."

  actual_hash="$(shasum -a 256 "$wasm" | awk '{print $1}')"
  [[ "$actual_hash" == "$EXPECTED_WASM_HASH" ]] \
    || die "V2 WASM hash mismatch: expected $EXPECTED_WASM_HASH, got $actual_hash."

  interface="$(stellar contract info interface --wasm "$wasm" --output json)"
  actual_interface_hash="$(printf '%s' "$interface" | jq -S -c . | shasum -a 256 | awk '{print $1}')"
  [[ "$actual_interface_hash" == "$EXPECTED_INTERFACE_HASH" ]] \
    || die "V2 interface hash mismatch: expected $EXPECTED_INTERFACE_HASH, got $actual_interface_hash."

  actual_functions="$(jq -r '.[].function_v0?.name // empty' <<<"$interface" | LC_ALL=C sort)"
  [[ "$actual_functions" == "$EXPECTED_FUNCTIONS" ]] \
    || die "V2 exported function set differs from the reviewed 18-function interface."

  actual_events="$(jq -r '.[].event_v0?.name // empty' <<<"$interface" | LC_ALL=C sort)"
  [[ "$actual_events" == "$EXPECTED_EVENTS" ]] \
    || die "V2 exported event set differs from the reviewed 9-event interface."

  echo "Reviewed V2 WASM verified."
  echo "  bytes: $EXPECTED_WASM_SIZE"
  echo "  SHA-256: $EXPECTED_WASM_HASH"
  echo "  interface SHA-256: $EXPECTED_INTERFACE_HASH"
  echo "  exports: 18 functions, 9 events"
}

xdr_hash() {
  local xdr_file="$1"
  local hash
  hash="$(stellar tx hash \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    --rpc-url https://rpc.invalid \
    "$xdr_file")"
  hash="$(tr -d '[:space:]' <<<"$hash")"
  assert_hash "Transaction hash" "$hash"
  printf '%s' "$hash"
}

decode_xdr_json() {
  local xdr_file="$1"
  local decoded
  decoded="$(stellar tx decode --output json "$xdr_file")" \
    || die "Stellar CLI could not decode the XDR."
  jq -e . <<<"$decoded" >/dev/null \
    || die "Stellar CLI returned an unrecognized decoded-XDR document."
  printf '%s' "$decoded"
}

require_exact_two_envelope_signatures() {
  local decoded="$1"
  local envelope_kind signatures count distinct_count
  envelope_kind="$(jq -r '
    if has("fee_bump") or has("tx_fee_bump") then "fee_bump"
    elif has("tx") and has("tx_v0") then "ambiguous"
    elif has("tx") then "tx"
    elif has("tx_v0") then "tx_v0"
    else "unknown"
    end
  ' <<<"$decoded")"
  case "$envelope_kind" in
    tx) signatures="$(jq -c '.tx.signatures // empty' <<<"$decoded")" ;;
    tx_v0) signatures="$(jq -c '.tx_v0.signatures // empty' <<<"$decoded")" ;;
    fee_bump)
      die "Fee-bump envelopes are not supported by this two-signature release workflow."
      ;;
    *) die "Decoded XDR is not one recognized transaction envelope." ;;
  esac

  jq -e '
    type == "array"
    and all(.[]; type == "object" and (.signature | type == "string" and length > 0))
  ' <<<"$signatures" >/dev/null \
    || die "Envelope signatures use an unrecognized shape. Submission is blocked."
  count="$(jq 'length' <<<"$signatures")"
  distinct_count="$(jq '[.[].signature] | unique | length' <<<"$signatures")"
  [[ "$count" == "2" && "$distinct_count" == "2" ]] \
    || die "Submission requires exactly 2 distinct envelope signatures; found $count signatures and $distinct_count distinct signature values."
}

require_zero_envelope_signatures() {
  local decoded="$1"
  local envelope_kind signatures count
  envelope_kind="$(jq -r '
    if has("fee_bump") or has("tx_fee_bump") then "fee_bump"
    elif has("tx") and has("tx_v0") then "ambiguous"
    elif has("tx") then "tx"
    elif has("tx_v0") then "tx_v0"
    else "unknown"
    end
  ' <<<"$decoded")"
  case "$envelope_kind" in
    tx) signatures="$(jq -c '.tx.signatures // empty' <<<"$decoded")" ;;
    tx_v0) signatures="$(jq -c '.tx_v0.signatures // empty' <<<"$decoded")" ;;
    fee_bump)
      die "The reviewed unsigned transaction cannot be a fee-bump envelope."
      ;;
    *) die "The reviewed unsigned XDR is not one recognized transaction envelope." ;;
  esac

  jq -e 'type == "array"' <<<"$signatures" >/dev/null \
    || die "The reviewed unsigned envelope has an unrecognized signature shape."
  count="$(jq 'length' <<<"$signatures")"
  [[ "$count" == "0" ]] \
    || die "The reviewed unsigned XDR must contain zero signatures; found $count."
}

build_contract() {
  [[ "$#" -eq 0 ]] || die "build does not accept arguments."
  reject_hidden_signing_inputs
  require_stellar_cli
  stellar contract build \
    --manifest-path "$CONTRACT_DIR/Cargo.toml" \
    --package mandate-registry \
    --locked \
    --optimize \
    --meta source_repo=github:ackrate/ackrate-protocol-contracts \
    --meta home_domain=ackrate.xyz
  verify_wasm "$DEFAULT_WASM"
}

verify_wasm_command() {
  local wasm="$DEFAULT_WASM"
  while (( $# > 0 )); do
    case "$1" in
      --wasm)
        require_value "$1" "${2:-}"
        wasm="$2"
        shift 2
        ;;
      *) die "Unknown verify-wasm argument: $1" ;;
    esac
  done
  reject_hidden_signing_inputs
  require_stellar_cli
  verify_wasm "$wasm"
}

prepare_upload() {
  local source="" rpc_url="" out="" wasm="$DEFAULT_WASM"
  while (( $# > 0 )); do
    case "$1" in
      --source|--rpc-url|--out|--wasm)
        require_value "$1" "${2:-}"
        case "$1" in
          --source) source="$2" ;;
          --rpc-url) rpc_url="$2" ;;
          --out) out="$2" ;;
          --wasm) wasm="$2" ;;
        esac
        shift 2
        ;;
      *) die "Unknown prepare-upload argument: $1" ;;
    esac
  done
  [[ -n "$source" && -n "$rpc_url" && -n "$out" ]] \
    || die "prepare-upload requires --source, --rpc-url, and --out."
  assert_g_address_shape "--source" "$source"
  assert_rpc_url "$rpc_url"
  assert_new_xdr_path "$out"
  reject_hidden_signing_inputs
  require_stellar_cli
  assert_valid_strkey "--source" "$source"
  verify_wasm "$wasm"

  local xdr
  xdr="$(stellar --quiet --no-cache contract upload \
    --rpc-url "$rpc_url" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    --source-account "$source" \
    --wasm "$wasm" \
    --optimize=false \
    --build-only)"
  write_new_xdr "$out" "$xdr"
}

prepare_add_signer() {
  local source="" signer="" rpc_url="" out=""
  while (( $# > 0 )); do
    case "$1" in
      --source|--signer|--rpc-url|--out)
        require_value "$1" "${2:-}"
        case "$1" in
          --source) source="$2" ;;
          --signer) signer="$2" ;;
          --rpc-url) rpc_url="$2" ;;
          --out) out="$2" ;;
        esac
        shift 2
        ;;
      *) die "Unknown prepare-add-signer argument: $1" ;;
    esac
  done
  [[ -n "$source" && -n "$signer" && -n "$rpc_url" && -n "$out" ]] \
    || die "prepare-add-signer requires --source, --signer, --rpc-url, and --out."
  assert_g_address_shape "--source" "$source"
  assert_g_address_shape "--signer" "$signer"
  [[ "$source" != "$signer" ]] || die "--signer must differ from the source account's master key."
  assert_rpc_url "$rpc_url"
  assert_new_xdr_path "$out"
  reject_hidden_signing_inputs
  require_stellar_cli
  assert_valid_strkey "--source" "$source"
  assert_valid_strkey "--signer" "$signer"

  local xdr
  xdr="$(stellar --quiet --no-cache tx new set-options \
    --rpc-url "$rpc_url" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    --source-account "$source" \
    --signer "$signer" \
    --signer-weight 1 \
    --build-only)"
  write_new_xdr "$out" "$xdr"
  echo "Sign and send this account-setup XDR in official Stellar Laboratory."
  echo "Do not use this workflow's two-signature submit command for initial account setup."
}

prepare_thresholds() {
  local source="" rpc_url="" out=""
  while (( $# > 0 )); do
    case "$1" in
      --source|--rpc-url|--out)
        require_value "$1" "${2:-}"
        case "$1" in
          --source) source="$2" ;;
          --rpc-url) rpc_url="$2" ;;
          --out) out="$2" ;;
        esac
        shift 2
        ;;
      *) die "Unknown prepare-thresholds argument: $1" ;;
    esac
  done
  [[ -n "$source" && -n "$rpc_url" && -n "$out" ]] \
    || die "prepare-thresholds requires --source, --rpc-url, and --out."
  assert_g_address_shape "--source" "$source"
  assert_rpc_url "$rpc_url"
  assert_new_xdr_path "$out"
  reject_hidden_signing_inputs
  require_stellar_cli
  assert_valid_strkey "--source" "$source"

  local xdr
  xdr="$(stellar --quiet --no-cache tx new set-options \
    --rpc-url "$rpc_url" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    --source-account "$source" \
    --master-weight 1 \
    --low-threshold 2 \
    --med-threshold 2 \
    --high-threshold 2 \
    --build-only)"
  write_new_xdr "$out" "$xdr"
  echo "Sign and send this account-setup XDR in official Stellar Laboratory."
  echo "Do not use this workflow's two-signature submit command for initial account setup."
}

preflight_authority() {
  local authority="" source="" signer_1="" signer_2="" signer_3="" horizon_url=""
  while (( $# > 0 )); do
    case "$1" in
      --authority|--source|--signer-1|--signer-2|--signer-3|--horizon-url)
        require_value "$1" "${2:-}"
        case "$1" in
          --authority) authority="$2" ;;
          --source) source="$2" ;;
          --signer-1) signer_1="$2" ;;
          --signer-2) signer_2="$2" ;;
          --signer-3) signer_3="$2" ;;
          --horizon-url) horizon_url="$2" ;;
        esac
        shift 2
        ;;
      *) die "Unknown preflight-authority argument: $1" ;;
    esac
  done
  [[ -n "$authority" && -n "$source" && -n "$signer_1" && -n "$signer_2" && -n "$signer_3" && -n "$horizon_url" ]] \
    || die "preflight-authority requires --authority, --source, --signer-1, --signer-2, --signer-3, and --horizon-url."
  assert_g_address_shape "--authority" "$authority"
  assert_g_address_shape "--source" "$source"
  assert_g_address_shape "--signer-1" "$signer_1"
  assert_g_address_shape "--signer-2" "$signer_2"
  assert_g_address_shape "--signer-3" "$signer_3"
  [[ "$authority" == "$signer_1" ]] \
    || die "--authority and --signer-1 must be the same master account."
  [[ "$source" == "$authority" ]] \
    || die "--source and --authority must be the same reviewed 2-of-3 account."
  [[ "$signer_1" != "$signer_2" && "$signer_1" != "$signer_3" && "$signer_2" != "$signer_3" ]] \
    || die "The three signers must be distinct public G-accounts."
  assert_horizon_url "$horizon_url"
  reject_hidden_signing_inputs
  require_stellar_cli
  command -v curl >/dev/null 2>&1 || die "curl is required for the authority preflight."
  command -v jq >/dev/null 2>&1 || die "jq is required for the authority preflight."
  assert_valid_strkey "--authority" "$authority"
  assert_valid_strkey "--source" "$source"
  assert_valid_strkey "--signer-1" "$signer_1"
  assert_valid_strkey "--signer-2" "$signer_2"
  assert_valid_strkey "--signer-3" "$signer_3"

  local base network_record authority_record source_record
  base="${horizon_url%/}"
  network_record="$(curl --fail --silent --show-error \
    --proto '=https' --proto-redir '=https' --max-redirs 0 \
    "$base/")" || die "Horizon network lookup failed."
  jq -e --arg passphrase "$NETWORK_PASSPHRASE" \
    '.network_passphrase == $passphrase' <<<"$network_record" >/dev/null \
    || die "Horizon does not identify the Stellar Mainnet network passphrase."

  authority_record="$(curl --fail --silent --show-error \
    --proto '=https' --proto-redir '=https' --max-redirs 0 \
    "$base/accounts/$authority")" || die "Authority account lookup failed."
  jq -e \
    --arg account "$authority" \
    --arg signer1 "$signer_1" \
    --arg signer2 "$signer_2" \
    --arg signer3 "$signer_3" '
      .account_id == $account
      and (.signers | type == "array" and length == 3)
      and (([.signers[].key] | sort) == ([$signer1, $signer2, $signer3] | sort))
      and all(.signers[]; .type == "ed25519_public_key" and .weight == 1)
      and .thresholds.low_threshold == 2
      and .thresholds.med_threshold == 2
      and .thresholds.high_threshold == 2
    ' <<<"$authority_record" >/dev/null \
    || die "Authority must contain exactly the three reviewed weight-1 Ed25519 signers with thresholds 2/2/2."

  source_record="$(curl --fail --silent --show-error \
    --proto '=https' --proto-redir '=https' --max-redirs 0 \
    "$base/accounts/$source")" || die "Source account lookup failed."
  jq -e --arg account "$source" '
    .account_id == $account
    and (.balances | type == "array")
    and any(.balances[];
      .asset_type == "native"
      and (.balance | type == "string")
      and ((try (.balance | tonumber) catch -1) > 0)
    )
  ' <<<"$source_record" >/dev/null \
    || die "Source account must exist and have a positive native XLM balance."

  echo "Mainnet authority preflight passed."
  echo "  authority: $authority"
  echo "  signers: 3 reviewed accounts (weight 1 each)"
  echo "  thresholds: 2/2/2"
  echo "  funded source: $source"
}

inspect_xdr() {
  local xdr_file=""
  while (( $# > 0 )); do
    case "$1" in
      --xdr)
        require_value "$1" "${2:-}"
        xdr_file="$2"
        shift 2
        ;;
      *) die "Unknown inspect-xdr argument: $1" ;;
    esac
  done
  [[ -n "$xdr_file" ]] || die "inspect-xdr requires --xdr."
  assert_xdr_file "$xdr_file"
  reject_hidden_signing_inputs
  require_stellar_cli
  echo "Transaction envelope:"
  stellar tx decode --output json-formatted "$xdr_file"
  echo
  echo "Mainnet transaction hash: $(xdr_hash "$xdr_file")"
  echo "Nothing was signed or broadcast."
}

lab_signing_guide() {
  local xdr_file=""
  while (( $# > 0 )); do
    case "$1" in
      --xdr)
        require_value "$1" "${2:-}"
        xdr_file="$2"
        shift 2
        ;;
      *) die "Unknown lab-signing-guide argument: $1" ;;
    esac
  done
  [[ -n "$xdr_file" ]] || die "lab-signing-guide requires --xdr."
  assert_xdr_file "$xdr_file"
  reject_hidden_signing_inputs
  require_stellar_cli
  decode_xdr_json "$xdr_file" >/dev/null
  local hash
  hash="$(xdr_hash "$xdr_file")"

  cat <<EOF
Stellar Laboratory two-signature handoff

Transaction hash: $hash
Existing XDR file: $xdr_file
Laboratory: $LAB_SIGN_URL

1. Run inspect-xdr and have both people compare the transaction hash and fields.
2. Open Stellar Laboratory, select Stellar Mainnet, and paste the existing raw XDR.
3. Connect signer 1 in Freighter. Sign only after Laboratory and the wallet show the reviewed transaction fields.
4. Copy/export the updated signed raw XDR into a NEW .xdr file. Do not overwrite the unsigned file.
5. Load signer 1's updated XDR in Laboratory. Connect signer 2 and sign it.
6. Copy/export the final two-signature raw XDR into another NEW .xdr file.
7. Run inspect-xdr on the final file. Then use submit with the exact hash-bound confirmation it prints.

Do not enter a secret key or seed phrase in this script, the shell, or the repository.
The CLI --sign-with-lab shortcut is deliberately not used: it couples the
Laboratory flow to sending and does not return the signed XDR to this workflow.

Raw XDR to paste:
$(<"$xdr_file")
EOF
}

submit_xdr() {
  local xdr_file="" reviewed_unsigned="" rpc_url="" confirmation=""
  while (( $# > 0 )); do
    case "$1" in
      --xdr|--reviewed-unsigned|--rpc-url|--confirm)
        require_value "$1" "${2:-}"
        case "$1" in
          --xdr) xdr_file="$2" ;;
          --reviewed-unsigned) reviewed_unsigned="$2" ;;
          --rpc-url) rpc_url="$2" ;;
          --confirm) confirmation="$2" ;;
        esac
        shift 2
        ;;
      *) die "Unknown submit argument: $1" ;;
    esac
  done
  [[ -n "$xdr_file" && -n "$reviewed_unsigned" && -n "$rpc_url" ]] \
    || die "submit requires --xdr, --reviewed-unsigned, and --rpc-url."
  assert_xdr_file "$xdr_file"
  assert_xdr_file "$reviewed_unsigned"
  [[ "$xdr_file" != "$reviewed_unsigned" ]] \
    || die "Signed and reviewed-unsigned XDR must be separate files."
  assert_rpc_url "$rpc_url"
  reject_hidden_signing_inputs
  require_stellar_cli
  command -v jq >/dev/null 2>&1 || die "jq is required to verify envelope signatures."
  local decoded reviewed_decoded hash reviewed_hash required_confirmation
  decoded="$(decode_xdr_json "$xdr_file")"
  require_exact_two_envelope_signatures "$decoded"
  reviewed_decoded="$(decode_xdr_json "$reviewed_unsigned")"
  require_zero_envelope_signatures "$reviewed_decoded"
  hash="$(xdr_hash "$xdr_file")"
  reviewed_hash="$(xdr_hash "$reviewed_unsigned")"
  [[ "$hash" == "$reviewed_hash" ]] \
    || die "Signed XDR does not match the exact reviewed unsigned transaction. Submission is blocked."
  required_confirmation="BROADCAST_MAINNET_${hash}"
  [[ "$confirmation" == "$required_confirmation" ]] || {
    echo "Submission guard is CLOSED." >&2
    echo "Reviewed XDR hash: $hash" >&2
    echo "To broadcast this exact file, pass:" >&2
    echo "  --confirm $required_confirmation" >&2
    echo "Nothing was broadcast." >&2
    exit 1
  }

  echo "FINAL MAINNET BROADCAST: $hash" >&2
  echo "Matched the exact reviewed unsigned transaction." >&2
  echo "Validated exactly two distinct envelope signatures." >&2
  stellar --quiet --no-cache tx send \
    --rpc-url "$rpc_url" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    "$xdr_file"
}

prepare_deploy() {
  local source="" admin="" initial_asset="" observed_hash="" rpc_url="" out="" wasm="$DEFAULT_WASM"
  while (( $# > 0 )); do
    case "$1" in
      --source|--admin|--initial-asset|--observed-wasm-hash|--rpc-url|--out|--wasm)
        require_value "$1" "${2:-}"
        case "$1" in
          --source) source="$2" ;;
          --admin) admin="$2" ;;
          --initial-asset) initial_asset="$2" ;;
          --observed-wasm-hash) observed_hash="$2" ;;
          --rpc-url) rpc_url="$2" ;;
          --out) out="$2" ;;
          --wasm) wasm="$2" ;;
        esac
        shift 2
        ;;
      *) die "Unknown prepare-deploy argument: $1" ;;
    esac
  done
  [[ -n "$source" && -n "$admin" && -n "$initial_asset" && -n "$observed_hash" && -n "$rpc_url" && -n "$out" ]] \
    || die "prepare-deploy requires --source, --admin, --initial-asset, --observed-wasm-hash, --rpc-url, and --out."
  assert_g_address_shape "--source" "$source"
  assert_g_address_shape "--admin" "$admin"
  [[ "$source" == "$admin" ]] \
    || die "--source and --admin must be the same reviewed 2-of-3 account."
  assert_c_address_shape "--initial-asset" "$initial_asset"
  [[ "$initial_asset" == "$CANONICAL_USDC_SAC" ]] \
    || die "--initial-asset must be the canonical Circle Stellar Mainnet USDC SAC: $CANONICAL_USDC_SAC"
  assert_hash "--observed-wasm-hash" "$observed_hash"
  [[ "$observed_hash" == "$EXPECTED_WASM_HASH" ]] \
    || die "Observed uploaded WASM hash does not equal the reviewed V2 hash. Deployment is blocked."
  assert_rpc_url "$rpc_url"
  assert_new_xdr_path "$out"
  reject_hidden_signing_inputs
  require_stellar_cli
  assert_valid_strkey "--source" "$source"
  assert_valid_strkey "--admin" "$admin"
  assert_valid_strkey "--initial-asset" "$initial_asset"
  verify_wasm "$wasm"

  local xdr
  xdr="$(stellar --quiet --no-cache contract deploy \
    --rpc-url "$rpc_url" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    --source-account "$source" \
    --wasm-hash "$observed_hash" \
    --build-only \
    -- \
    --admin "$admin" \
    --initial-asset "$initial_asset")"
  write_new_xdr "$out" "$xdr"
}

verify_deploy() {
  local contract_id="" source="" admin="" initial_asset="" rpc_url=""
  while (( $# > 0 )); do
    case "$1" in
      --contract-id|--source|--admin|--initial-asset|--rpc-url)
        require_value "$1" "${2:-}"
        case "$1" in
          --contract-id) contract_id="$2" ;;
          --source) source="$2" ;;
          --admin) admin="$2" ;;
          --initial-asset) initial_asset="$2" ;;
          --rpc-url) rpc_url="$2" ;;
        esac
        shift 2
        ;;
      *) die "Unknown verify-deploy argument: $1" ;;
    esac
  done
  [[ -n "$contract_id" && -n "$source" && -n "$admin" && -n "$initial_asset" && -n "$rpc_url" ]] \
    || die "verify-deploy requires --contract-id, --source, --admin, --initial-asset, and --rpc-url."
  assert_c_address_shape "--contract-id" "$contract_id"
  assert_g_address_shape "--source" "$source"
  assert_g_address_shape "--admin" "$admin"
  assert_c_address_shape "--initial-asset" "$initial_asset"
  [[ "$initial_asset" == "$CANONICAL_USDC_SAC" ]] \
    || die "--initial-asset must be the canonical Circle Stellar Mainnet USDC SAC: $CANONICAL_USDC_SAC"
  assert_rpc_url "$rpc_url"
  reject_hidden_signing_inputs
  require_stellar_cli
  command -v jq >/dev/null 2>&1 || die "jq is required for post-deploy state verification."
  assert_valid_strkey "--contract-id" "$contract_id"
  assert_valid_strkey "--source" "$source"
  assert_valid_strkey "--admin" "$admin"
  assert_valid_strkey "--initial-asset" "$initial_asset"

  local observed_hash observed_admin observed_pending observed_schema observed_paused observed_asset_allowed
  observed_hash="$(stellar --quiet --no-cache contract info hash \
    --rpc-url "$rpc_url" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    --contract-id "$contract_id")"
  observed_hash="$(tr -d '[:space:]' <<<"$observed_hash")"
  [[ "$observed_hash" == "$EXPECTED_WASM_HASH" ]] \
    || die "STOP: deployed contract hash mismatch. Expected $EXPECTED_WASM_HASH, observed $observed_hash."

  observed_admin="$(stellar --quiet --no-cache contract invoke \
    --rpc-url "$rpc_url" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    --source-account "$source" \
    --id "$contract_id" \
    --send=no \
    -- get_admin)" || die "Read-only get_admin verification failed."
  jq -e --arg expected "$admin" 'type == "string" and . == $expected' \
    <<<"$observed_admin" >/dev/null \
    || die "STOP: deployed get_admin does not equal the expected administrator."

  observed_pending="$(stellar --quiet --no-cache contract invoke \
    --rpc-url "$rpc_url" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    --source-account "$source" \
    --id "$contract_id" \
    --send=no \
    -- get_pending_admin)" || die "Read-only get_pending_admin verification failed."
  jq -e 'type == "null"' <<<"$observed_pending" >/dev/null \
    || die "STOP: deployed contract unexpectedly has a pending administrator."

  observed_schema="$(stellar --quiet --no-cache contract invoke \
    --rpc-url "$rpc_url" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    --source-account "$source" \
    --id "$contract_id" \
    --send=no \
    -- get_schema_version)" || die "Read-only get_schema_version verification failed."
  jq -e 'type == "number" and . == 2' <<<"$observed_schema" >/dev/null \
    || die "STOP: deployed schema version is not 2."

  observed_paused="$(stellar --quiet --no-cache contract invoke \
    --rpc-url "$rpc_url" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    --source-account "$source" \
    --id "$contract_id" \
    --send=no \
    -- is_paused)" || die "Read-only is_paused verification failed."
  jq -e 'type == "boolean" and . == false' <<<"$observed_paused" >/dev/null \
    || die "STOP: newly deployed contract is not initially unpaused."

  observed_asset_allowed="$(stellar --quiet --no-cache contract invoke \
    --rpc-url "$rpc_url" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    --source-account "$source" \
    --id "$contract_id" \
    --send=no \
    -- is_asset_allowed \
    --asset "$initial_asset")" || die "Read-only is_asset_allowed verification failed."
  jq -e 'type == "boolean" and . == true' <<<"$observed_asset_allowed" >/dev/null \
    || die "STOP: canonical Circle USDC is not allowed by the deployed contract."

  echo "Deployed V2 contract verified."
  echo "  contract: $contract_id"
  echo "  WASM SHA-256: $observed_hash"
  echo "  admin: $admin"
  echo "  pending admin: none"
  echo "  schema: 2"
  echo "  paused: false"
  echo "  canonical Circle USDC allowed: true"
}

command_name="${1:-help}"
if (( $# > 0 )); then
  shift
fi

case "$command_name" in
  help|-h|--help) usage ;;
  build) build_contract "$@" ;;
  verify-wasm) verify_wasm_command "$@" ;;
  prepare-add-signer) prepare_add_signer "$@" ;;
  prepare-thresholds) prepare_thresholds "$@" ;;
  prepare-upload) prepare_upload "$@" ;;
  preflight-authority) preflight_authority "$@" ;;
  inspect-xdr) inspect_xdr "$@" ;;
  lab-signing-guide) lab_signing_guide "$@" ;;
  submit) submit_xdr "$@" ;;
  prepare-deploy) prepare_deploy "$@" ;;
  verify-deploy) verify_deploy "$@" ;;
  *)
    usage >&2
    die "Unknown command: $command_name"
    ;;
esac
