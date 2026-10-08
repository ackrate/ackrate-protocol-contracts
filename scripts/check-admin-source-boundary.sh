#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
tracked=$(git ls-files -- tools/admin/ui/ tools/admin/dist/)
if [[ -n "$tracked" ]]; then
  printf '%s\n' 'Operator frontend source and bundles belong to the private control repository.' >&2
  printf '%s\n' "$tracked" >&2
  exit 1
fi
