import { readFile } from "node:fs/promises";
import { createHash, randomBytes } from "node:crypto";
import {
  Address,
  Contract,
  Keypair,
  Networks,
  Operation,
  StrKey,
  TransactionBuilder,
  nativeToScVal,
  rpc,
  scValToNative,
  xdr,
} from "@stellar/stellar-sdk";

const RPC_URL = "https://soroban-testnet.stellar.org";
const HORIZON_URL = "https://horizon-testnet.stellar.org";
const WASM_PATH = new URL("../public/mandate_registry.wasm", import.meta.url);
const server = new rpc.Server(RPC_URL);
const deployer = Keypair.random();
const adminMaster = Keypair.random();
const secondary1 = Keypair.random();
const secondary2 = Keypair.random();
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function waitFor(hash) {
  for (let attempt = 0; attempt < 45; attempt += 1) {
    await sleep(2000);
    const result = await server.getTransaction(hash);
    if (result.status === "SUCCESS") return result;
    if (result.status === "FAILED") throw new Error(`Testnet transaction failed: ${hash}`);
  }
  throw new Error(`Timed out waiting for testnet transaction: ${hash}`);
}

async function submit(operations, source, signingKeys, prepare = true) {
  const account = await server.getAccount(source.publicKey());
  const builder = new TransactionBuilder(account, {
    fee: String(100 * operations.length),
    networkPassphrase: Networks.TESTNET,
  }).setTimeout(300);
  operations.forEach((operation) => builder.addOperation(operation));
  let transaction = builder.build();
  if (prepare) transaction = await server.prepareTransaction(transaction);
  signingKeys.forEach((key) => transaction.sign(key));
  const sent = await server.sendTransaction(transaction);
  if (sent.status === "ERROR") throw new Error("RPC rejected a testnet smoke transaction.");
  return waitFor(sent.hash);
}

async function readPolicy(account) {
  const key = xdr.LedgerKey.account(new xdr.LedgerKeyAccount({
    accountId: Keypair.fromPublicKey(account).xdrAccountId(),
  }));
  const result = await server.getLedgerEntries(key);
  const entry = result.entries[0].val.account();
  return {
    thresholds: [...entry.thresholds()],
    signers: entry.signers().map((signer) => ({
      key: StrKey.encodeEd25519PublicKey(signer.key().ed25519()),
      weight: signer.weight(),
    })),
  };
}

async function simulateRead(contractId, method) {
  const account = await server.getAccount(deployer.publicKey());
  const transaction = new TransactionBuilder(account, {
    fee: "100",
    networkPassphrase: Networks.TESTNET,
  }).addOperation(new Contract(contractId).call(method)).setTimeout(300).build();
  const simulation = await server.simulateTransaction(transaction);
  if (!rpc.Api.isSimulationSuccess(simulation) || !simulation.result?.retval) {
    throw new Error(`${method} simulation failed.`);
  }
  return simulation.result.retval;
}

const network = await server.getNetwork();
if (network.passphrase !== Networks.TESTNET) throw new Error("RPC is not testnet.");
await Promise.all([
  server.fundAddress(deployer.publicKey()),
  server.fundAddress(adminMaster.publicKey()),
]);
for (const account of [deployer.publicKey(), adminMaster.publicKey()]) {
  const horizonAccount = await fetch(`${HORIZON_URL}/accounts/${account}`);
  if (!horizonAccount.ok) throw new Error("Funded account is not visible through testnet Horizon.");
}

const policyTransaction = await submit([
  Operation.setOptions({ signer: { ed25519PublicKey: secondary1.publicKey(), weight: 1 } }),
  Operation.setOptions({ signer: { ed25519PublicKey: secondary2.publicKey(), weight: 1 } }),
  Operation.setOptions({ masterWeight: 1, lowThreshold: 2, medThreshold: 2, highThreshold: 2 }),
], adminMaster, [adminMaster], false);
const policy = await readPolicy(adminMaster.publicKey());
if (
  policy.thresholds.join(",") !== "1,2,2,2"
  || policy.signers.length !== 2
  || policy.signers.some((signer) => signer.weight !== 1)
) throw new Error("2-of-3 policy verification failed.");

const wasm = new Uint8Array(await readFile(WASM_PATH));
const wasmHash = createHash("sha256").update(wasm).digest();
const upload = await submit([Operation.uploadContractWasm({ wasm })], deployer, [deployer]);
const deployment = await submit([Operation.createCustomContract({
  address: new Address(deployer.publicKey()),
  wasmHash,
  salt: randomBytes(32),
  constructorArgs: [new Address(adminMaster.publicKey()).toScVal()],
})], deployer, [deployer]);
if (!deployment.returnValue) throw new Error("Deployment returned no contract ID.");
const contractId = Address.fromScVal(deployment.returnValue).toString();
const contract = new Contract(contractId);

const observedAdmin = Address.fromScVal(await simulateRead(contractId, "get_admin")).toString();
if (observedAdmin !== adminMaster.publicKey()) throw new Error("Constructor admin mismatch.");
const pause = await submit([contract.call("pause")], adminMaster, [adminMaster, secondary1]);
if (scValToNative(await simulateRead(contractId, "is_paused")) !== true) throw new Error("Pause failed.");
const unpause = await submit([contract.call("unpause")], adminMaster, [secondary1, secondary2]);
if (scValToNative(await simulateRead(contractId, "is_paused")) !== false) throw new Error("Unpause failed.");
const upgrade = await submit([
  contract.call("upgrade", nativeToScVal(wasmHash, { type: "bytes" })),
], adminMaster, [adminMaster, secondary2]);

process.stdout.write(`${JSON.stringify({
  network: "testnet",
  rpc: RPC_URL,
  horizon: HORIZON_URL,
  deployer: deployer.publicKey(),
  adminAccount: adminMaster.publicKey(),
  secondarySigners: [secondary1.publicKey(), secondary2.publicKey()],
  policy: { masterLowMediumHigh: policy.thresholds, signerWeights: policy.signers },
  policyTransaction: policyTransaction.txHash,
  wasmSha256: wasmHash.toString("hex"),
  uploadTransaction: upload.txHash,
  contractId,
  deploymentTransaction: deployment.txHash,
  pauseTransaction: pause.txHash,
  unpauseTransaction: unpause.txHash,
  selfUpgradeTransaction: upgrade.txHash,
}, null, 2)}\n`);
