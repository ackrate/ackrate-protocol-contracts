# Canary controller fixture

`timelock-canary-v0.1.0.wasm` is the deployed Mainnet controller code fetched by
its ledger code hash. SHA-256:
`99a32170feaf3521338adfadb25d1a2ea573e6d29ec5de97e9d9cc3e4a99da97`.
The deployment manifest records its source, release and contract address.

These historical registry tests use the deployed ABI. Revised controller source
uses the V2 admin API and has its own compatibility tests. This fixture is test
input and is not deployed by the administration tooling.
