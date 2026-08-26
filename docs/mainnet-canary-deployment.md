# Mainnet canary deployment

The Ackrate governed canary was deployed to Stellar mainnet on
2026-08-26. It contains one payment contract and one governance contract:

- [MandateRegistry](https://stellar.expert/explorer/public/contract/CDBTG5ZKASFA7LOYUPBOTGKAVX5MJIM4U24BYGX7VX23IHYDAHLQPAGS) is the only mandate and USDC payment path.
- [TimelockController](https://stellar.expert/explorer/public/contract/CD3KRQRNCW52CZHKG2GPQAEOU6UCL426YFNHYUZ7IWUUKAOTKUQX6UUX) governs Registry administration, asset policy, and upgrades.

The authority path is `2-of-3 Stellar account -> TimelockController ->
MandateRegistry`. The Timelock is self-administered, has a minimum delay of
17,280 ledgers, grants proposer and canceller roles to the 2-of-3 account, and
has no executor allowlist. Therefore an approved operation may be executed by
anyone only after the delay. The Registry is initially unpaused and allows only
the canonical Circle Stellar mainnet USDC SAC.

## Deployment transactions

| Action | Ledger | Transaction |
|---|---:|---|
| Upload Timelock WASM | 64132883 | [`cba62b05...0b73`](https://stellar.expert/explorer/public/tx/cba62b052c68350013f7e430cff939af6b23bb6dadc4e419135293832e570b73) |
| Create Timelock | 64132885 | [`ecc8becb...5213`](https://stellar.expert/explorer/public/tx/ecc8becb4473454d99811c90dd7bcf81175833717063f69c30438f847f065213) |
| Upload MandateRegistry WASM | 64132887 | [`2f1de955...b4f7`](https://stellar.expert/explorer/public/tx/2f1de95547d3309b3c48140160627690f88ec561dfdad43dab92e4d500e8b4f7) |
| Create MandateRegistry | 64132889 | [`0d7e6891...3c05`](https://stellar.expert/explorer/public/tx/0d7e689188f58d9a68b0ef0c08f0fed3aaac5033e618240f8f7a7bd28e073c05) |

All four Horizon records report `successful: true`. The complete timestamps,
fees, constructor arguments, public authorities, contract IDs, and links are in
[`contracts/mainnet/deployment-manifest.json`](../contracts/mainnet/deployment-manifest.json).

## Source and bytecode evidence

| Contract | Observed mainnet WASM SHA-256 | Source evidence |
|---|---|---|
| TimelockController | `99a32170feaf3521338adfadb25d1a2ea573e6d29ec5de97e9d9cc3e4a99da97` | [release](https://github.com/ackrate/ackrate-protocol-contracts/releases/tag/mainnet-canary-v0.1.0_contracts_mainnet_timelock_controller_ackrate-timelock-controller_pkg0.1.0_cli27.0.0), [attestation](https://github.com/ackrate/ackrate-protocol-contracts/attestations/43110888) |
| MandateRegistry | `3656430ac7cf5e7cf1c26948b46314c37866c2d7e928ea89d7d1f89b8aa0ef3c` | [release](https://github.com/ackrate/ackrate-protocol-contracts/releases/tag/mainnet-canary-v0.1.0_contracts_mainnet_mandate_registry_mandate-registry_pkg0.3.0_cli27.0.0), [attestation](https://github.com/ackrate/ackrate-protocol-contracts/attestations/43110909) |

The official build is [run 32963207324](https://github.com/ackrate/ackrate-protocol-contracts/actions/runs/32963207324).
The exact-byte independent rebuild is [run 32966147370](https://github.com/ackrate/ackrate-protocol-contracts/actions/runs/32966147370).

The live MandateRegistry bytecode embeds
`source_repo: github:ackrate/ackrate-protocol-contracts` and
`home_domain: ackrate.xyz`. Its source mapping is the exact repository commit
[`51b9315`](https://github.com/ackrate/ackrate-protocol-contracts/commit/51b93159d5a4e29d9e48fe99f489d70271703494),
path `contracts/mainnet/mandate-registry`, package `mandate-registry`, and
[successful build job](https://github.com/ackrate/ackrate-protocol-contracts/actions/runs/32963207324/job/98160946789).
The downloaded release artifact has SHA-256
`3656430ac7cf5e7cf1c26948b46314c37866c2d7e928ea89d7d1f89b8aa0ef3c`,
identical to the live mainnet WASM. GitHub attestation verification passes with
the source digest and tag pinned and the StellarExpert reusable builder pinned.

## Independent read-only verification

Post-deployment reads verified:

- both live WASM hashes equal the attested release hashes;
- Timelock delay is 17,280 ledgers;
- Timelock admin is its own contract ID;
- the authority has proposer and canceller roles;
- executor role member count is zero, making execution permissionless only after the delay;
- Registry admin is the Timelock;
- Registry roles are `pauser`, `unpauser`, `assetpol`, and `upgrader`;
- the emergency key is the sole pauser;
- the 2-of-3 authority is the sole unpauser;
- the Timelock is the sole asset-policy authority and upgrader;
- Registry schema version is 1 and it is unpaused; and
- Circle USDC SAC `CCW67TSZ...MI75` is allowed.

These reads were simulated with `stellar contract invoke --send=no`; no
verification transaction or state mutation was submitted.
