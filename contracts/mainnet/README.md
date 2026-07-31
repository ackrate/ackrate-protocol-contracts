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

## Prepare the deployment record

Copy `deployment-manifest.template.json` outside the repository into the
private deployment evidence location. Complete every `null` field from
independently verified evidence. Do not place signer secrets, recovery
material, API keys, or seed phrases in the manifest or repository.

The deployment runner reads only public addresses or local identity aliases.
Review the runner and completed manifest together before use:

```bash
./scripts/deploy-mainnet.sh
```

With no environment configuration, it prints the required inputs and exits.
It refuses to submit unless `REAPP_MAINNET_CONFIRM=DEPLOY_EXACT_REVIEWED_BUILD`
is set. Setting that guard is an operational authorization step, not a
substitute for review or approval.

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
