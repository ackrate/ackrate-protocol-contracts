#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RELEASE_DIR="$ROOT/target/mainnet-release"
RUN_ID="${ACKRATE_MAINNET_ARTIFACT_RUN_ID:-}"

if [[ ! "$RUN_ID" =~ ^[1-9][0-9]*$ ]]; then
  echo "Set ACKRATE_MAINNET_ARTIFACT_RUN_ID to a successful Contract Gate Check run on main." >&2
  exit 2
fi
if ! command -v gh >/dev/null 2>&1; then
  echo "GitHub CLI is required to fetch the reviewed release artifact." >&2
  exit 3
fi
if [[ -n "$(git -C "$ROOT" status --porcelain)" ]]; then
  echo "Refusing to fetch release artifacts into a dirty worktree." >&2
  exit 4
fi

current_commit="$(git -C "$ROOT" rev-parse HEAD)"
IFS=$'\t' read -r status conclusion event head_sha head_branch workflow_name \
  < <(gh run view "$RUN_ID" \
    --json status,conclusion,event,headSha,headBranch,workflowName \
    --jq '[.status,.conclusion,.event,.headSha,.headBranch,.workflowName] | @tsv')

if [[ "$status" != "completed" || "$conclusion" != "success" ]]; then
  echo "The selected GitHub run did not complete successfully." >&2
  exit 5
fi
if [[ "$event" != "push" || "$head_sha" != "$current_commit" || "$head_branch" != "main" ]]; then
  echo "The selected GitHub run is not for this exact main-branch commit." >&2
  exit 6
fi
if [[ "$workflow_name" != "Contract Gate Check" ]]; then
  echo "The selected GitHub run is not the governed contract workflow." >&2
  exit 7
fi

download_dir="$(mktemp -d)"
trap 'rm -rf -- "$download_dir"' EXIT
artifact_name="mainnet-release-$current_commit"
gh run download "$RUN_ID" --name "$artifact_name" --dir "$download_dir"
node "$ROOT/scripts/check-mainnet-artifacts.mjs" --release-dir "$download_dir"

mkdir -p "$RELEASE_DIR"
for filename in \
  ackrate_timelock_controller.wasm \
  mandate_registry.wasm \
  ackrate_timelock_controller.interface.json \
  mandate_registry.interface.json \
  SHA256SUMS \
  SIZES; do
  install -m 0644 "$download_dir/$filename" "$RELEASE_DIR/$filename"
done

node "$ROOT/scripts/check-mainnet-artifacts.mjs"
echo "Verified canonical mainnet artifacts installed from GitHub run $RUN_ID."
