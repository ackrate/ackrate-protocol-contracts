# Mainnet MandateRegistry deployment

The governed canary is live on Stellar mainnet. The canonical public record is
[`deployment-manifest.json`](deployment-manifest.json), with a human-readable
[deployment and verification report](../../docs/mainnet-canary-deployment.md).

- TimelockController: [`CD3KRQRN...6UUX`](https://stellar.expert/explorer/public/contract/CD3KRQRNCW52CZHKG2GPQAEOU6UCL426YFNHYUZ7IWUUKAOTKUQX6UUX)
- MandateRegistry: [`CDBTG5ZK...PAGS`](https://stellar.expert/explorer/public/contract/CDBTG5ZKASFA7LOYUPBOTGKAVX5MJIM4U24BYGX7VX23IHYDAHLQPAGS)

This directory contains the two contracts used by the governed mainnet
deployment profile:

- `mandate-registry`: the sole mandate validation, state-consumption, and
  payment path; and
- `timelock-controller`: the canonical OpenZeppelin-based delay authority.

The optional reference policy under `contracts/extensions` is not part of this
deployment profile. The registry has no extension registry, callback, plugin
hook, or extension-specific storage.

## Governance profile

The canary timelock is deployed first with:

- the live native Stellar 2-of-3 account as proposer and canceller;
- no bootstrap administrator;
- no executor list, making execution permissionless after the delay; and
- the reviewed minimum delay in ledgers.

MandateRegistry is then deployed with:

- the timelock as top administrator, asset-policy authority, and upgrader;
- a separate emergency pauser that can only stop payments;
- the live 2-of-3 account as unpauser; and
- the independently derived and verified mainnet USDC Stellar Asset Contract
  as the initial allowed asset.

The on-chain 2-of-3 signer math is already live: exactly three Ed25519 signers,
weight 1 each, and low/medium/high thresholds of 2. The remaining step is the
independent physical Freighter custody handoff. The canary deployment record
tracks that ceremony separately from the technical multisig state.

## Reproduce the release artifacts

Run:

```bash
./scripts/gatecheck-mainnet.sh
```

The script formats, lints, tests, and builds both contracts, inspects their
interfaces, and writes the exact WASM files plus SHA-256 checksums beneath
`target/mainnet-release/`. Generated files are intentionally not committed.

The reviewed canary artifact was built on Ubuntu 24.04 x86_64 with the tagged
source's pinned Rust 1.96.0 toolchain
and Stellar CLI 27.0.0. The source-verification release uses the pinned
StellarExpert build workflow and embeds the canonical Ackrate repository and
home-domain metadata. The currently reviewed candidate hashes are recorded in
`deployment-manifest.template.json`. A hash mismatch is a release stop.
The same gate runs continuously on pushes and pull requests to `main`.

The `mainnet-canary-v0.1.0` source tag produced the reviewed canary artifacts
through the official StellarExpert v27.0.0 builder. Both release assets have
GitHub SLSA provenance signed for source commit
`51b93159d5a4e29d9e48fe99f489d70271703494`, witnessed by the Sigstore
transparency log, and verified locally against GitHub CLI's signed trust root:

| Artifact | SHA-256 | Release | Attestation |
|---|---|---|---|
| Timelock Controller | `99a32170feaf3521338adfadb25d1a2ea573e6d29ec5de97e9d9cc3e4a99da97` | [v0.1.0 WASM](https://github.com/ackrate/ackrate-protocol-contracts/releases/tag/mainnet-canary-v0.1.0_contracts_mainnet_timelock_controller_ackrate-timelock-controller_pkg0.1.0_cli27.0.0) | [GitHub provenance](https://github.com/ackrate/ackrate-protocol-contracts/attestations/43110888) |
| MandateRegistry | `3656430ac7cf5e7cf1c26948b46314c37866c2d7e928ea89d7d1f89b8aa0ef3c` | [v0.3.0 WASM](https://github.com/ackrate/ackrate-protocol-contracts/releases/tag/mainnet-canary-v0.1.0_contracts_mainnet_mandate_registry_mandate-registry_pkg0.3.0_cli27.0.0) | [GitHub provenance](https://github.com/ackrate/ackrate-protocol-contracts/attestations/43110909) |

[View the successful source build and attestation run](https://github.com/ackrate/ackrate-protocol-contracts/actions/runs/32963207324).

Host operating systems can produce different WASM bytes even with equal file
sizes and pinned tools. The workflow therefore publishes the exact canonical
files only after the Ubuntu gate reproduces the reviewed hashes. On another
platform, fetch the artifact for the current clean commit from a successful
`main` run, then verify it locally:

```bash
export ACKRATE_MAINNET_ARTIFACT_RUN_ID=<successful-github-run-id>
./scripts/fetch-mainnet-artifacts.sh
```

The fetcher rejects failed runs, other branches, other commits, other
workflows, dirty worktrees, and any artifact whose hash or size differs from
the candidate manifest.

Circle's current Stellar mainnet issuer was reverified against Circle's
published asset page. Stellar CLI 26.1.0 and `@stellar/stellar-sdk`
independently derive the same Stellar Asset Contract:

```text
USDC:GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN
CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75
```

## Deployment record

`deployment-manifest.json` contains only public mainnet evidence. It was
completed from independently verified Horizon, RPC, StellarExpert, release,
and attestation records. It contains no signer secrets, recovery material, API
keys, or seed phrases. `deployment-manifest.template.json` remains the input
for future deployments.

The deployment runner reads only public addresses or local identity aliases.
Review the runner and completed manifest together before use:

```bash
export ACKRATE_DEPLOYER=<local-stellar-cli-identity-alias>
export ACKRATE_DEPLOYMENT_SOURCE_ACCOUNT=<public-G-address-for-that-alias>
export ACKRATE_AUTHORITY_2_OF_3=<public-G-address>
export ACKRATE_AUTHORITY_MANIFEST=<absolute-path-to-public-authority-manifest.json>
export ACKRATE_EMERGENCY_PAUSER=<public-G-address>
export ACKRATE_MAINNET_RPC_URL=<credential-free-https-rpc-url>
# Optional; keep private and never add it to the deployment manifest or repo:
export ACKRATE_MAINNET_RPC_HEADER='X-API-Key: <provider-secret>'
export ACKRATE_MAINNET_USDC_SAC=CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75
export ACKRATE_TIMELOCK_DELAY_LEDGERS=17280

node ./scripts/preflight-mainnet.mjs
./scripts/deploy-mainnet.sh
```

With no environment configuration, it prints the required inputs and exits.
It refuses to submit unless `ACKRATE_MAINNET_CONFIRM=DEPLOY_EXACT_REVIEWED_BUILD`
is set. Setting that guard is an operational authorization step, not a
substitute for review or approval.

The preflight is read-only. It checks the RPC health, deployment source,
Circle USDC identity, the public authority manifest, and the live Stellar
account thresholds. Every A+B, A+C, and B+C pair must satisfy both medium and
high thresholds while every single signer remains insufficient. The deployment
preflight proves on-chain signer math only; the independent physical custody
handoff is recorded separately in the completed manifest. The deployment
runner accepts only a local identity alias and verifies that its public key is
the recorded deployment source; secrets and seed phrases are not accepted as
arguments or environment values. If the selected RPC provider requires an
access header, it may be supplied only through `ACKRATE_MAINNET_RPC_HEADER`; the
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
