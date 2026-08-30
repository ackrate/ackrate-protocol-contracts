# SimpleContract mainnet deployment and self-upgrade

This runbook prepares a Stellar **2-of-3 G-account**, deploys the Simple
MandateRegistry with that account as admin, and later upgrades the contract at
the same contract ID. It never asks signers to share secret keys.

The current SimpleContract has one upgrade method:

```text
upgrade(new_wasm_hash)
```

It calls `require_auth()` on the stored admin and immediately replaces the
current WASM. There is no contract-level schedule, cancellation, delay, or pause
gate. The 2-of-3 account threshold is therefore the upgrade policy.

## Fixed inputs

Use Stellar CLI `27.0.0` and record all public values in a release ticket.

```sh
export NETWORK_PASSPHRASE='Public Global Stellar Network ; September 2015'
export RPC_URL='https://YOUR-TRUSTED-MAINNET-RPC'
export ADMIN_ACCOUNT='GA2B3YY27OY6AWT2VXMXUDBSAHVOLU2ST6QWJJJLOIGDQHJDXO4RL4XH'
export SIGNER_2='G...'
export SIGNER_3='G...'
export DEPLOYER='local-stellar-cli-identity-name'
```

`ADMIN_ACCOUNT` is the existing project owner public key recorded by the Simple
release. Confirm it with the owner before funding or submitting anything. Each
other signer supplies their own `G...` public key over an authenticated,
out-of-band channel. Never put an `S...` seed in a shell variable, document,
chat, website, CI secret used by a public job, or browser form.

Validate the three keys are different:

```sh
for key in "$ADMIN_ACCOUNT" "$SIGNER_2" "$SIGNER_3"; do
  [[ "$key" =~ ^G[A-Z2-7]{55}$ ]] || exit 1
done
[[ "$ADMIN_ACCOUNT" != "$SIGNER_2" ]]
[[ "$ADMIN_ACCOUNT" != "$SIGNER_3" ]]
[[ "$SIGNER_2" != "$SIGNER_3" ]]
```

## 1. Create and fund the admin account

Create the account whose master public key is `ADMIN_ACCOUNT` and fund it with
enough XLM for fees plus the two signer subentries. Each signer subentry raises
the account minimum balance by one base reserve. Check the current reserve and
the account on two independent explorers/RPC providers before continuing.

Do not set thresholds in the account-creation transaction. First prove that the
account exists, is funded, and is controlled by the expected master key.

## 2. Add signers, then set 2-of-3 thresholds

Order matters. Submit and verify each transaction before building the next, so
each uses the current account sequence. The owner signs these three setup
transactions with their normal offline/Freighter workflow.

```sh
stellar tx new set-options \
  --source-account "$ADMIN_ACCOUNT" \
  --signer "$SIGNER_2" --signer-weight 1 \
  --rpc-url "$RPC_URL" --network-passphrase "$NETWORK_PASSPHRASE"

stellar tx new set-options \
  --source-account "$ADMIN_ACCOUNT" \
  --signer "$SIGNER_3" --signer-weight 1 \
  --rpc-url "$RPC_URL" --network-passphrase "$NETWORK_PASSPHRASE"

stellar tx new set-options \
  --source-account "$ADMIN_ACCOUNT" \
  --master-weight 1 --low-threshold 2 --med-threshold 2 --high-threshold 2 \
  --rpc-url "$RPC_URL" --network-passphrase "$NETWORK_PASSPHRASE"
```

When using `--source-account` as a public key, add the CLI's preferred signing
option (`--sign-with-key`, `--sign-with-ledger`, or `--sign-with-lab`) rather
than exposing a seed. After the final transaction, query the selected Stellar
RPC with `getLedgerEntries` for the account ledger key, decode the returned
`AccountEntry` XDR, and require:

- master key weight `1`;
- signer 2 weight `1`;
- signer 3 weight `1`;
- low, medium, and high thresholds all `2`;
- no unknown signers.

Stop if any value differs. A threshold of `1` is not 2-of-3. Do not disable the
master key; it is one of the three intended signers.

## 3. Reproducibly build and inspect SimpleContract

From the repository root:

```sh
rustup toolchain install 1.98.0 --profile minimal --component clippy,rustfmt \
  --target wasm32v1-none
./scripts/gatecheck-contracts.sh
cargo build --locked --release --target wasm32v1-none \
  --manifest-path contracts/simple/mandate-registry/Cargo.toml
export SIMPLE_WASM='contracts/simple/mandate-registry/target/wasm32v1-none/release/mandate_registry.wasm'
shasum -a 256 "$SIMPLE_WASM"
stellar contract inspect --wasm "$SIMPLE_WASM"
```

Two reviewers compare the commit, toolchain, interface (especially the sole
`upgrade` entry point), file size, and SHA-256. Tag the exact reviewed commit.

## 4. Deploy with the multisig admin

The deployment source pays fees; it need not be the admin. Deploy only the
reviewed WASM and pass the 2-of-3 account to the constructor:

```sh
export CONTRACT_ID="$(stellar contract deploy \
  --source-account "$DEPLOYER" \
  --wasm "$SIMPLE_WASM" \
  --rpc-url "$RPC_URL" --network-passphrase "$NETWORK_PASSPHRASE" \
  -- --admin "$ADMIN_ACCOUNT")"

stellar contract invoke \
  --source-account "$DEPLOYER" --id "$CONTRACT_ID" \
  --rpc-url "$RPC_URL" --network-passphrase "$NETWORK_PASSPHRASE" \
  -- get_admin
```

Require `get_admin` to equal `ADMIN_ACCOUNT`. Record the contract ID, deployment
transaction, ledger, source commit, build commands, WASM SHA-256, uploaded WASM
hash, and verification links. Exercise read-only calls before accepting funds.

## 5. Prepare a same-address upgrade

Build the replacement from its reviewed tag. Uploading code does not change the
contract and can be paid by the deployer:

```sh
export NEW_WASM='path/to/reviewed-mandate_registry.wasm'
shasum -a 256 "$NEW_WASM"
export NEW_WASM_HASH="$(stellar contract upload \
  --source-account "$DEPLOYER" --wasm "$NEW_WASM" \
  --rpc-url "$RPC_URL" --network-passphrase "$NETWORK_PASSPHRASE")"
```

Confirm the returned hash against the reviewed artifact. Freeze release inputs.
Open the published Upgrade Signer, enter `RPC_URL`, `ADMIN_ACCOUNT`,
`CONTRACT_ID`, and `NEW_WASM_HASH`, then press **Build and simulate via RPC**.
The app reads the source account, validates the RPC network passphrase, builds
the sole `upgrade(BytesN<32>)` invocation, and prepares its Soroban resources by
simulation. All network interaction uses that RPC endpoint.

The XDR is time- and sequence-sensitive. Do not submit any other transaction
from `ADMIN_ACCOUNT` while signatures are being collected. If it expires or the
sequence changes, discard every copy and build a new envelope.

## 6. Collect two signatures

The prepared transaction creates a self-contained cosigner URL. Its fragment
contains the RPC URL and complete envelope XDR; URL fragments are not sent to
Vercel or another application server. Send this URL to signer A over the agreed
channel. Signer A verifies the fields and appends a Freighter signature, then
copies the newly generated URL to signer B. Signer B repeats the same process.
Existing signatures remain attached to each next URL.

Both signers independently verify:

- network is Public Global Stellar Network;
- source is `ADMIN_ACCOUNT`;
- the transaction hash agreed out of band;
- there is exactly one contract invocation;
- contract is `CONTRACT_ID`;
- function is `upgrade`;
- argument is `NEW_WASM_HASH`;
- fee, sequence, and validity window are expected.

The app never accepts a secret key. It decodes locally and asks Freighter to
append a signature. The link is durable and serverless, but its embedded
transaction still expires and becomes invalid if the source sequence advances.
Rebuild rather than editing an expired transaction.

## 7. Submit once and verify

After the UI reports two attached signatures, either signer or the coordinator
presses **Submit through RPC**. The browser calls `sendTransaction` and polls
`getTransaction` on the exact RPC embedded in the URL until success or failure.
There is no Horizon, application API, database, relayer, or Vercel function in
the prepare, signing, sharing, submission, or confirmation path.

Never add a third signature: surplus valid signatures can produce
`TX_BAD_AUTH_EXTRA`. Wait for final success, then verify the executable hash at
the unchanged contract ID, the admin, pause state, and representative mandates.
Run smoke tests against reads first, then one bounded payment. Record all results.

Rollback is another reviewed `upgrade(previous_wasm_hash)` transaction signed
by any two of the three signers. Prepare and upload the rollback artifact before
the maintenance window, but never pre-sign a rollback transaction whose sequence
or contents may become stale.

## Incident stops

Stop immediately if a signer or contract ID differs, decoded operations contain
anything extra, the hash changes between reviewers, an unknown signer appears,
the account sequence moves, simulation changes, or the post-upgrade executable
hash differs. Do not “fix” an envelope manually; rebuild and restart review.
