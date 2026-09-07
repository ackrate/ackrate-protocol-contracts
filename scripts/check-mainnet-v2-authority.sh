#!/usr/bin/env bash
set -euo pipefail

# Read-only check of a Horizon account JSON on stdin. A legitimate key rotation
# requires a reviewed update to this baseline and the private custodian register;
# do not weaken the exact-key check merely to make the monitor pass.
jq --exit-status --slurp '
  length == 1 and (.[0] |
    .account_id == "GCIURCX7JHEKQLRTW6RDZU7OJUVCDM7WWNQPIKRERIHQOHSLW7UY7TXG" and
    .thresholds.low_threshold == 2 and
    .thresholds.med_threshold == 2 and
    .thresholds.high_threshold == 2 and
    (.signers | type) == "array" and
    (.signers | length) == 3 and
    all(.signers[]; .type == "ed25519_public_key" and .weight == 1) and
    ([.signers[].key] | sort) == ([
      "GCIURCX7JHEKQLRTW6RDZU7OJUVCDM7WWNQPIKRERIHQOHSLW7UY7TXG",
      "GD3UEYYZRU53VBAVGEKR6HYQ3USQ3FEBT5BLOYEX356EFOM5SR5774GW",
      "GD57LQEI6PLLDWT5TVKUYNTKHRPEZEGRYK7OCJUSUP767PE2P7MXE73B"
    ] | sort)
  )
' >/dev/null
