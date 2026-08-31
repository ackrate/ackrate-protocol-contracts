# Mainnet V2 source verification

Status as of 2026-08-31 16:18 UTC: the deployed bytecode is reproducibly
matched to this repository and has canonical GitHub provenance, but
StellarExpert still reports `validation.status = unverified`. The remaining
failure is at StellarExpert's public validation-queue intake, after the source,
byte-for-byte build, release, and attestation checks have succeeded.

## Deployment identity

| Field | Value |
|---|---|
| Network | Stellar Public Network |
| Contract | [`CCLZEBJXG4YVJEPBCR5F27N733BCK5HQJWZZGB3K54JVODY3VAGP4HWR`](https://stellar.expert/explorer/public/contract/CCLZEBJXG4YVJEPBCR5F27N733BCK5HQJWZZGB3K54JVODY3VAGP4HWR) |
| On-chain WASM SHA-256 | `982809197d35d44c7b0fce6bd117fb2fec09b728c64c146c1f803b01faacff62` |
| Source repository | `https://github.com/ackrate/ackrate-protocol-contracts` |
| Source directory | `contracts/mainnet-v2/mandate-registry` |
| Cargo package | `mandate-registry` |
| Package version | `0.4.1` |
| Source commit | [`02a01b89638b291ff6e03697ec98d175cc117b59`](https://github.com/ackrate/ackrate-protocol-contracts/commit/02a01b89638b291ff6e03697ec98d175cc117b59) |
| Verification run | [`33412027817`](https://github.com/ackrate/ackrate-protocol-contracts/actions/runs/33412027817) |
| GitHub attestation | [`44188903`](https://github.com/ackrate/ackrate-protocol-contracts/attestations/44188903) |
| Release artifact | [`mandate-registry_v0.4.1.wasm`](https://github.com/ackrate/ackrate-protocol-contracts/releases/download/v2-source-verify-v0.4.1.5_contracts_mainnet_v2_mandate_registry_mandate-registry_pkg0.4.1_cli27.0.0/mandate-registry_v0.4.1.wasm) |
| Upstream incident | [`stellar-expert/soroban-build-workflow#9`](https://github.com/stellar-expert/soroban-build-workflow/issues/9) |

The deployed WASM embeds both expected metadata entries:

```text
source_repo=github:ackrate/ackrate-protocol-contracts
home_domain=ackrate.xyz
```

## What passed

The `v2-source-verify-v0.4.1.5` run used the official reusable workflow at the
canonical builder identity:

```text
https://github.com/stellar-expert/soroban-build-workflow/.github/workflows/release.yml@refs/heads/main
```

Before invoking it, the repository gate checked that `main` still resolved to
the reviewed workflow commit
`88068ec50cba931a96436869727ed08edeb76ade`. The run then:

1. installed Rust `1.98.0`, the `wasm32v1-none` target, and Stellar CLI
   `27.0.0` from a SHA-256-pinned release archive;
2. rebuilt `contracts/mainnet-v2/mandate-registry` with the same optimization
   and metadata arguments used for the deployment;
3. ran the reviewed test suite, including the ignored release-byte test;
4. required the rebuilt artifact to equal the deployed hash
   `98280919...faacff62` exactly;
5. published a GitHub release whose asset digest is the same hash; and
6. generated SLSA provenance tying that exact artifact to source commit
   `02a01b89...117b59` and the official StellarExpert builder.

The exact-source-and-byte gate and official release/attestation job both
completed successfully. This establishes the source-to-bytecode relationship
independently of the explorer label.

## Where StellarExpert failed

The official workflow sent this documented match request before creating its
attestation:

```json
{
  "repository": "https://github.com/ackrate/ackrate-protocol-contracts",
  "commitHash": "02a01b89638b291ff6e03697ec98d175cc117b59",
  "jobId": "build",
  "runId": "33412027817",
  "contractHash": "982809197d35d44c7b0fce6bd117fb2fec09b728c64c146c1f803b01faacff62",
  "relativePath": "contracts/mainnet-v2/mandate-registry",
  "packageName": "mandate-registry"
}
```

The repository workflow then waited until GitHub's attestation API returned the
new proof and resubmitted the same match. It made 20 attempts at 30-second
intervals. Every request returned HTTP 200 with `{}`. The historical accepted
response is `{"ok":1}`. The workflow deliberately failed rather than
misrepresenting an empty HTTP response as successful queue insertion.

The public contract API continued to return:

```json
{
  "wasm": "982809197d35d44c7b0fce6bd117fb2fec09b728c64c146c1f803b01faacff62",
  "validation": {"status": "unverified"}
}
```

This rules out a slow build or late attestation: the retry began only after the
proof was readable and continued for more than ten minutes.

## Mainnet control: Reflector

The closest working comparison is Reflector's repository-built Pulse Oracle,
not Circle's USDC asset contract:

| Field | Reflector control |
|---|---|
| Contract | [`CALI2BYU2JE6WVRUFYTS6MSBNEHGJ35P4AVCZYF3B6QOE3QKOB2PLE6M`](https://stellar.expert/explorer/public/contract/CALI2BYU2JE6WVRUFYTS6MSBNEHGJ35P4AVCZYF3B6QOE3QKOB2PLE6M) |
| Repository | `https://github.com/reflector-network/reflector-contract` |
| Commit | `42f3116b0c5ea335181508c926e5ec48d67af419` |
| Package | `reflector-pulse-contract` |
| Verification run | [`30000081080`](https://github.com/reflector-network/reflector-contract/actions/runs/30000081080) |
| WASM hash | `8ecd1857496df2c15aaab4d18d2d7689542a62814245e9b2c613c609b86bd11c` |

Reflector used the same official reusable workflow, CLI `27.0.0`, SLSA builder
identity, GitHub attestation mechanism, and public match endpoint. Its original
run received `{"ok":1}`, and StellarExpert now returns
`validation.status = verified` with the repository, commit, and package above.

As a control, the exact already-verified Reflector payload was resubmitted on
2026-08-31. The endpoint returned HTTP 200 with `{}` instead of the historical
`{"ok":1}`. That makes an Ackrate-specific source or payload defect unlikely
and points to current intake behavior.

## Why Circle is different

Circle's Stellar USDC contract
[`CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75`](https://stellar.expert/explorer/public/contract/CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75)
is the native Stellar Asset Contract for
`USDC:GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN`. StellarExpert
classifies it through its `asset` field. It does not expose the repository
verification object used for repository-built WASM contracts. Circle's
[`stablecoin-xlm`](https://github.com/circlefin/stablecoin-xlm) repository is
therefore not the mechanism behind a Soroban source-verification badge for that
contract. Reflector is the applicable mainnet implementation pattern.

## Root-cause assessment

Confirmed facts:

- the on-chain and rebuilt WASM hashes are identical;
- the release asset digest is identical;
- GitHub serves canonical SLSA provenance for the exact digest;
- the source commit, repository, package, and relative path are public;
- the official StellarExpert builder completed successfully; and
- the match endpoint returned `{}` from fresh GitHub-hosted runners and for a
  known-good Reflector control.

The leading service-side hypothesis is a failure in the GitHub Actions source-IP
allowlist or queue deduplication/reprocessing path. A publicly indexed,
non-authoritative copy of the validation module shows the route returning
`{ok: 1}` only after an IP check and otherwise falling through without a value.
That copy also contains suspicious range-refresh code. Because it is not an
official production source publication, this is evidence for investigation,
not proof of the deployed implementation. The details and control results are
recorded in upstream issue #9.

There is no documented authenticated public endpoint for deleting a stale queue
row, forcing reprocessing, or directly writing `contract_code_source`. Trying
undocumented destructive methods would bypass StellarExpert's trust boundary
and would not constitute legitimate verification. The explorer operator must
repair the intake allowlist or reprocess the existing hash.

## Recovery and acceptance criteria

1. StellarExpert fixes its GitHub Actions IP/range handling or reprocesses the
   queue record for `98280919...faacff62`.
2. Re-run `.github/workflows/source-verify-mainnet-v2.yml` from a unique
   `v2-source-verify-v*` tag if a fresh submission is requested. The post-proof
   job must receive `{"ok":1}`.
3. Accept verification only when the public API returns all of:

   ```json
   {
     "status": "verified",
     "repository": "https://github.com/ackrate/ackrate-protocol-contracts",
     "commit": "02a01b89638b291ff6e03697ec98d175cc117b59",
     "package": "mandate-registry"
   }
   ```

4. Confirm the explorer displays `Source code: ackrate/ackrate-protocol-contracts`
   and that the link resolves to this repository.

The contract is not to be redeployed merely to change an explorer label. The
current contract address already points to the proven artifact, and a new
deployment would not repair a broken verifier intake.

## Independent verification

```bash
curl -LO https://github.com/ackrate/ackrate-protocol-contracts/releases/download/v2-source-verify-v0.4.1.5_contracts_mainnet_v2_mandate_registry_mandate-registry_pkg0.4.1_cli27.0.0/mandate-registry_v0.4.1.wasm
sha256sum mandate-registry_v0.4.1.wasm
gh attestation verify mandate-registry_v0.4.1.wasm \
  --repo ackrate/ackrate-protocol-contracts
curl -fsS \
  https://api.stellar.expert/explorer/public/contract/CCLZEBJXG4YVJEPBCR5F27N733BCK5HQJWZZGB3K54JVODY3VAGP4HWR \
  | jq '{wasm, validation}'
```

The expected file digest and API `wasm` value are both
`982809197d35d44c7b0fce6bd117fb2fec09b728c64c146c1f803b01faacff62`.
