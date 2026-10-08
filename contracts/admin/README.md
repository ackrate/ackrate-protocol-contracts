# Administration tools and Hello proof

The controller lives in `contracts/mainnet/timelock-controller`. This workspace
contains one Hello World contract with the V2 administrator and pause API.
It keeps a greeting and stored value through an upgrade. The proof reapplies the
same reviewed Hello WASM to the same instance. It verifies delayed authorization
and preserved state. No factory or second Hello implementation is used.

## Build and deployment

Run `scripts/gatecheck-admin.sh --with-deployed-v2`. CI builds the controller and
Hello with locked Rust 1.98.0 and Stellar CLI 28.1.0. It checks deployed V2 compatibility,
dependencies and JavaScript, then publishes an exact-source artifact.
Mainnet tools require that successful GitHub build and both reviewed WASM hashes.

The operator frontend is maintained in the private project control repository.
It uses these shared deployment and transaction-validation modules. The CLI and
proof below remain available from this repository.

## Standalone proof

Create a dedicated Stellar CLI secure-storage identity and fund its public address.
No secret key is accepted by these tools. Mainnet always uses 17,280 ledgers and
a 50-XLM per-transaction fee cap. Supply an existing failed Mainnet Soroban transaction
with diagnostic events to verify that early-execution errors can be recorded.

```sh
node tools/admin/proof.mjs start --network mainnet \
  --identity ackrate-timelock-mainnet-proof --out private/mainnet-proof \
  --diagnostic-probe "$FAILED_SOROBAN_TX" \
  --build-run "$BUILD_RUN" --build-commit "$REVIEWED_COMMIT"
# After both recorded ready ledgers:
node tools/admin/proof.mjs finish --network mainnet \
  --identity ackrate-timelock-mainnet-proof --out private/mainnet-proof
```

Start uploads only Hello and the controller, deploys both, nominates administration
and accepts management. It schedules reapplication of Hello WASM, proves duplicate
prevention and a submitted early rejection, pauses immediately, and schedules unpause
with another submitted early rejection. Finish executes the two actions after their
delays, checks Hello state and rejects replay. Testnet can use `--delay 90`.
Testnet defaults to the same fee cap. An explicit `--testnet-max-fee-stroops`
can raise it up to 2,000,000,000 stroops (200 Testnet XLM) after quoting the
transaction. Mainnet rejects this option and retains its fixed 50-XLM cap.

The proof journals each transaction hash before submission. A timeout requires a
receipt query. It never retries an uncertain transaction. Keep the private run
directory and selected GitHub artifact when resuming. Prior factory-proof receipts
must remain in their original directory. Use a new directory for this proof format.

## Unsigned administration packets

```sh
node tools/admin/admin.mjs prepare schedule --network testnet \
  --source "$GOVERNANCE" --controller "$CONTROLLER" \
  --action '["Upgrade","<64-hex-WASM-hash>"]' \
  --salt "$SALT" --out private/schedule.json
```

Use `prepare execute` with the original action, salt and predecessor. Other methods
are `cancel --id`, `pause_now`, `upload-helper`, `deploy-helper --target --delay --salt`
and pre-adoption `propose-governance --successor`. Sequential handoff methods
`propose-admin`, `accept-management` and `accept-governance` take `--file` deployment
JSON. Mainnet also requires `--governance` as the separately trusted recipient.
Their source must match the required signer in that file. Prepared packets expire after
30 minutes. Independently review the exact body before each custodian signs it.
Attach signed XDR with `attach`, then submit with `submit`. Keep packets and keys
in ignored private storage.

The controller is immutable. Replacement requires its delayed administrator
nomination and acceptance on a fresh instance. Stop renewing a retired instance
when it is no longer needed. Shared code can remain active for other instances.
Paid Soroban storage rent is not refundable.
