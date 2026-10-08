# Test fixtures

These reviewed WASM files are inputs for controller authorization and state tests.

- `hello.wasm`: the existing Hello contract, SHA-256 `92933ea27374242a45c2f97c4a47ffa1631063b156ae3e89cdcb74afc79145a4`.
- `mainnet-v2.wasm`: the deployed V2 registry, SHA-256 `982809197d35d44c7b0fce6bd117fb2fec09b728c64c146c1f803b01faacff62`.

The tests execute the current controller WASM from this crate's `target/` directory.
Build it before running `cargo clippy` or `cargo test`.
