# Mainnet MandateRegistry release candidate

This directory contains the two contracts used by the governed mainnet
deployment profile:

- `mandate-registry`: the sole mandate validation, state-consumption, and
  payment path; and
- `timelock-controller`: the canonical OpenZeppelin-based delay authority.

The optional reference policy under `contracts/extensions` is not part of this
deployment profile. The registry has no extension registry, callback, plugin
hook, or extension-specific storage.

## Governance profile

The timelock is deployed first with:

- the selected native Stellar 2-of-3 account as proposer and canceller;
- no bootstrap administrator;
- no executor list, making execution permissionless after the delay; and
- the reviewed minimum delay in ledgers.

MandateRegistry is then deployed with:

- the timelock as top administrator, asset-policy authority, and upgrader;
- a separate emergency pauser that can only stop payments;
- the 2-of-3 account as unpauser; and
- the independently derived and verified mainnet USDC Stellar Asset Contract
  as the initial allowed asset.

## Reproduce the release artifacts

Run:

```bash
./scripts/gatecheck-mainnet.sh
```

The script formats, lints, tests, and builds both contracts, inspects their
interfaces, and writes the exact WASM files plus SHA-256 checksums beneath
`target/mainnet-release/`. Generated files are intentionally not committed.

The currently reviewed candidate hashes are recorded in
`deployment-manifest.template.json`. A hash mismatch is a release stop.
The same gate runs continuously on pushes and pull requests to `main`.

Circle's current Stellar mainnet issuer was reverified against Circle's
published asset page. Stellar CLI 26.1.0 and `@stellar/stellar-sdk`
independently derive the same Stellar Asset Contract:

```text
USDC:GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN
CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75
```

## Prepare the deployment record

Copy `deployment-manifest.template.json` outside the repository into the
private deployment evidence location. Complete every `null` field from
independently verified evidence. Do not place signer secrets, recovery
material, API keys, or seed phrases in the manifest or repository.

The deployment runner reads only public addresses or local identity aliases.
Review the runner and completed manifest together before use:

```bash
export REAPP_DEPLOYER=<local-stellar-cli-identity-alias>
export REAPP_DEPLOYMENT_SOURCE_ACCOUNT=<public-G-address-for-that-alias>
export REAPP_AUTHORITY_2_OF_3=<public-G-address>
export REAPP_AUTHORITY_MANIFEST=<absolute-path-to-public-authority-manifest.json>
export REAPP_EMERGENCY_PAUSER=<public-G-address>
export REAPP_MAINNET_RPC_URL=<credential-free-https-rpc-url>
# Optional; keep private and never add it to the deployment manifest or repo:
export REAPP_MAINNET_RPC_HEADER='X-API-Key: <provider-secret>'
export REAPP_MAINNET_USDC_SAC=CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75
export REAPP_TIMELOCK_DELAY_LEDGERS=17280

node ./scripts/preflight-mainnet.mjs
./scripts/deploy-mainnet.sh
```

With no environment configuration, it prints the required inputs and exits.
It refuses to submit unless `REAPP_MAINNET_CONFIRM=DEPLOY_EXACT_REVIEWED_BUILD`
is set. Setting that guard is an operational authorization step, not a
substitute for review or approval.

The preflight is read-only. It checks the RPC health, deployment source,
Circle USDC identity, the public authority manifest, and the live Stellar
account thresholds. Every A+B, A+C, and B+C pair must satisfy both medium and
high thresholds while every single signer remains insufficient. The deployment
runner accepts only a local identity alias and verifies that its public key is
the recorded deployment source; secrets and seed phrases are not accepted as
arguments or environment values. If the selected RPC provider requires an
access header, it may be supplied only through `REAPP_MAINNET_RPC_HEADER`; the
runner passes it to the provider without printing or recording it.

The initial timelock must be at least 17,280 ledgers, approximately 24 hours at
Stellar's target ledger cadence. A longer reviewed delay is allowed. A shorter
delay is a release stop.

## Post-deployment verification

The runner fails unless the deployed contract hashes equal the locally
reviewed hashes. The deployment record must then capture:

- both transaction hashes and contract IDs;
- observed on-chain WASM hashes;
- the network passphrase and deployment ledger/time;
- the exact constructor arguments;
- the timelock delay and authority configuration;
- the verified USDC issuer-to-SAC derivation evidence;
- independent read-only role and state checks; and
- the final source commit.

Do not publish a contract ID or update SDK configuration until those checks
agree with the completed deployment manifest.
