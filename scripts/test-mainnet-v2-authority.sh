#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CHECK="$ROOT/scripts/check-mainnet-v2-authority.sh"
# These are public governance identities, not wallet secrets. The explicit
# fixture is independent of the validator so changing its baseline is reviewed.
fixture='{"account_id":"GCIURCX7JHEKQLRTW6RDZU7OJUVCDM7WWNQPIKRERIHQOHSLW7UY7TXG","thresholds":{"low_threshold":2,"med_threshold":2,"high_threshold":2},"signers":[{"key":"GCIURCX7JHEKQLRTW6RDZU7OJUVCDM7WWNQPIKRERIHQOHSLW7UY7TXG","type":"ed25519_public_key","weight":1},{"key":"GD3UEYYZRU53VBAVGEKR6HYQ3USQ3FEBT5BLOYEX356EFOM5SR5774GW","type":"ed25519_public_key","weight":1},{"key":"GD57LQEI6PLLDWT5TVKUYNTKHRPEZEGRYK7OCJUSUP767PE2P7MXE73B","type":"ed25519_public_key","weight":1}]}'
passed=0
accept() {
  printf '%s\n' "$1" | bash "$CHECK"
  passed=$((passed + 1))
}
reject() {
  if printf '%s\n' "$2" | bash "$CHECK" >/dev/null 2>&1; then
    echo "Authority check incorrectly accepted: $1" >&2
    exit 1
  fi
  passed=$((passed + 1))
}
mutate() { jq -c "$1" <<<"$fixture"; }

accept "$fixture"
accept "$(mutate '.signers |= reverse')"
preauth="$(mutate '.signers += [{key:"TAIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRDYM5",type:"preauth_tx",weight:2}]')"
hash_signer="$(mutate '.signers += [{key:"XAJBEEQSCIJBEEQSCIJBEEQSCIJBEEQSCIJBEEQSCIJBEEQSCIJBFMBD",type:"sha256_hash",weight:2}]')"
# Demonstrate that these well-formed synthetic signer identities reached the
# old monitor false-pass, rather than rejection by an unrelated precondition.
for unsafe in "$preauth" "$hash_signer"; do
  jq --exit-status '
    ([.signers[] | select(.type == "ed25519_public_key")] | length) == 3 and
    ([.signers[] | select(.type == "ed25519_public_key") | .weight] | sort) == [1, 1, 1]
  ' <<<"$unsafe" >/dev/null
done
reject 'additional weight-two preauthorized transaction signer' "$preauth"
reject 'additional weight-two hash signer' "$hash_signer"
reject 'additional Ed25519 signer' "$(mutate '.signers += [{key:"GBTM2YELSKFYRZIOB37KUM726HCDZ37AOKKLBOD6T7QKXJVDZ53DG4MA",type:"ed25519_public_key",weight:1}]')"
reject 'substituted signer' "$(mutate '.signers[2].key = "GBTM2YELSKFYRZIOB37KUM726HCDZ37AOKKLBOD6T7QKXJVDZ53DG4MA"')"
reject 'missing signer' "$(mutate '.signers |= .[0:2]')"
reject 'duplicate signer' "$(mutate '.signers[2] = .signers[1]')"
reject 'wrong account' "$(mutate '.account_id = "G-other"')"
for field in low_threshold med_threshold high_threshold; do
  reject "reduced $field" "$(mutate ".thresholds.$field = 1")"
  reject "raised $field" "$(mutate ".thresholds.$field = 3")"
done
for index in 0 1 2; do
  reject "zero signer weight $index" "$(mutate ".signers[$index].weight = 0")"
  reject "elevated signer weight $index" "$(mutate ".signers[$index].weight = 2")"
  reject "non-Ed25519 signer $index" "$(mutate ".signers[$index].type = \"preauth_tx\"")"
done
reject 'string weight' "$(mutate '.signers[0].weight = "1"')"
reject 'string threshold' "$(mutate '.thresholds.med_threshold = "2"')"
reject 'missing fields' '{}'
reject 'null account' 'null'
reject 'empty input' ''
reject 'malformed JSON' '{'
reject 'multiple account documents' "$fixture
$fixture"

echo "Mainnet V2 authority offline checks passed ($passed scenarios): exact account, keys, signer types/weights, thresholds and complete JSON."
