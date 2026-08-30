#!/usr/bin/env bash
set -euo pipefail

route_tag() {
  case "$1" in
    mainnet-v2*)
      echo "mainnet-v2 release tags are reserved but disabled until a dedicated V2 release path is reviewed" >&2
      return 1
      ;;
    simple-v*|v0.1.*) echo "simple" ;;
    composites-v*) echo "composites" ;;
    mainnet-canary-v*|mainnet-v*) echo "mainnet-bundle" ;;
    mainnet-registry-v*) echo "mainnet-registry" ;;
    source-verify-registry-v*) echo "source-registry" ;;
    source-verify-v*) echo "source-bundle" ;;
    *)
      echo "unsupported or ambiguous release tag: $1" >&2
      return 1
      ;;
  esac
}

if [[ "${1:-}" == "--self-test" ]]; then
  cases=(
    "simple-v1.2.3:simple"
    "v0.1.9:simple"
    "composites-v1.2.3:composites"
    "mainnet-canary-v1.2.3:mainnet-bundle"
    "mainnet-v1.2.3:mainnet-bundle"
    "mainnet-registry-v1.2.3:mainnet-registry"
    "source-verify-v1.2.3:source-bundle"
    "source-verify-registry-v1.2.3:source-registry"
  )
  for item in "${cases[@]}"; do
    tag="${item%%:*}"
    expected="${item#*:}"
    actual="$(route_tag "$tag")"
    [[ "$actual" == "$expected" ]] || {
      echo "release tag route mismatch for $tag: $actual" >&2
      exit 1
    }
  done

  rejected=("v0.4.1" "mainnet-v2-v0.4.1" "mainnet-v2" "unknown-v1")
  for tag in "${rejected[@]}"; do
    if route_tag "$tag" >/dev/null 2>&1; then
      echo "unsafe release tag was accepted: $tag" >&2
      exit 1
    fi
  done
  echo "Release tag routing passed: accepted tags are unambiguous and V2 releases fail closed."
  exit 0
fi

[[ $# -eq 1 ]] || {
  echo "usage: $0 <tag> | --self-test" >&2
  exit 2
}
route_tag "$1"
