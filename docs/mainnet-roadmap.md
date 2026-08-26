# ACKRATE Mainnet Delivery Roadmap

The canonical cross-repository roadmap is maintained in
[`ackrate-protocol/docs/mainnet-roadmap.md`](https://github.com/ackrate/ackrate-protocol/blob/992e1a70035bf324ad942ed947e83265dfb5cca8/docs/mainnet-roadmap.md).
The link is pinned to the exact planning commit so its content cannot drift.

This repository owns the on-chain work described in Work packages 2 and 3 and
Gate 1 of that roadmap:

- select and prove the 2-of-3 authority compatible with Soroban authorization;
- integrate OpenZeppelin Stellar access control and one canonical timelock;
- remove bootstrap and alternate privileged paths;
- activate only the independently verified mainnet USDC Stellar Asset Contract;
- preserve atomic validation, mandate consumption, and token transfer;
- align storage TTL behavior with mandate expiry;
- version storage and prove migration with representative records;
- cover every contract function with positive and negative cases;
- test unauthorized callers and roles, expiry, overspend, replay, re-entry,
  token failure, pause, rotation, and upgrade boundaries;
- produce reproducible optimized WASM, checksums, SBOM, scan evidence, and
  deployment metadata.

The governed canary deployment now exists on Stellar mainnet. Its exact
artifact, authority, timelock, USDC identity, constructor arguments, observed
chain state, contract IDs, and transaction links are published in
[`contracts/mainnet/deployment-manifest.json`](../contracts/mainnet/deployment-manifest.json)
and the [deployment report](mainnet-canary-deployment.md).
