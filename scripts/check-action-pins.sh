#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CANONICAL_VALIDATION_WORKFLOW="$ROOT/.github/workflows/source-verify-mainnet-v2.yml"
CANONICAL_VALIDATOR='stellar-expert/soroban-build-workflow/.github/workflows/release.yml@main'

is_immutable_uses_value() {
  local value="$1"
  case "$value" in
    ./*) return 0 ;;
    docker://*@sha256:*)
      local digest="${value##*@sha256:}"
      [[ "$digest" =~ ^[0-9a-fA-F]{64}$ ]]
      ;;
    *@*)
      local ref="${value##*@}"
      [[ "$ref" =~ ^[0-9a-fA-F]{40}$ ]]
      ;;
    *) return 1 ;;
  esac
}

self_test() {
  local full_sha='0123456789abcdef0123456789abcdef01234567'
  local full_digest='0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef'
  local accepted=(
    "actions/checkout@$full_sha"
    "owner/repository/.github/workflows/release.yml@$full_sha"
    "./.github/actions/local"
    "docker://alpine@sha256:$full_digest"
  )
  local rejected=(
    'owner/action@main'
    'owner/action@develop'
    'owner/action@v4'
    'owner/action@deadbeef'
    'owner/action'
    'docker://alpine:latest'
    'docker://alpine@sha256:deadbeef'
  )
  local value
  for value in "${accepted[@]}"; do
    is_immutable_uses_value "$value" || {
      echo "immutable action reference rejected by self-test: $value" >&2
      return 1
    }
  done
  for value in "${rejected[@]}"; do
    if is_immutable_uses_value "$value"; then
      echo "mutable action reference accepted by self-test: $value" >&2
      return 1
    fi
  done
}

self_test

failed=0
canonical_validator_count=0
while IFS= read -r workflow; do
  while IFS= read -r value; do
    if [[ "$workflow" == "$CANONICAL_VALIDATION_WORKFLOW" && "$value" == "$CANONICAL_VALIDATOR" ]]; then
      canonical_validator_count=$((canonical_validator_count + 1))
      continue
    fi
    if ! is_immutable_uses_value "$value"; then
      echo "$workflow: external action is not pinned to an immutable digest: $value" >&2
      failed=1
    fi
  done < <(
    sed -n -E \
      's/^[[:space:]]*(-[[:space:]]*)?uses:[[:space:]]*([^#[:space:]]+).*$/\2/p' \
      "$workflow"
  )
done < <(find "$ROOT/.github/workflows" -type f \( -name '*.yml' -o -name '*.yaml' \) -print)

if [[ "$canonical_validator_count" -ne 1 ]]; then
  echo "The isolated StellarExpert source-validation workflow must contain exactly one canonical @main builder reference." >&2
  failed=1
fi

if [[ "$failed" -ne 0 ]]; then
  exit 1
fi

echo "GitHub Action pin check passed: every normal external action uses an immutable digest; the single StellarExpert canonical identity is isolated behind an exact-head and exact-byte gate."
