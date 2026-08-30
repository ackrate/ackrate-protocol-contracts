import { readFile } from "node:fs/promises";
import { createHash, randomBytes } from "node:crypto";
import {
  Address,
  Contract,
  Keypair,
  Networks,
  Operation,
  TransactionBuilder,
  nativeToScVal,
  rpc,
} from "@stellar/stellar-sdk";

const RPC_URL = "https://soroban-testnet.stellar.org";
const HORIZON_URL = "https://horizon-testnet.stellar.org";
const WASM_PATH = new URL("../public/mandate_registry.wasm", import.meta.url);
const server = new rpc.Server(RPC_URL);
const signer = Keypair.random();

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

async function submit(operation) {
  const account = await server.getAccount(signer.publicKey());
  const transaction = new TransactionBuilder(account, {
    fee: "100",
    networkPassphrase: Networks.TESTNET,
  }).addOperation(operation).setTimeout(300).build();
  const prepared = await server.prepareTransaction(transaction);
  prepared.sign(signer);
  const sent = await server.sendTransaction(prepared);
  if (sent.status === "ERROR") throw new Error("RPC rejected a testnet smoke transaction.");
  return waitFor(sent.hash);
}

const network = await server.getNetwork();
if (network.passphrase !== Networks.TESTNET) throw new Error("RPC is not testnet.");
await server.fundAddress(signer.publicKey());
const horizonAccount = await fetch(`${HORIZON_URL}/accounts/${signer.publicKey()}`);
if (!horizonAccount.ok) throw new Error("Funded account is not visible through testnet Horizon.");

const wasm = new Uint8Array(await readFile(WASM_PATH));
const wasmHash = createHash("sha256").update(wasm).digest();
const upload = await submit(Operation.uploadContractWasm({ wasm }));
const deployment = await submit(Operation.createCustomContract({
  address: new Address(signer.publicKey()),
  wasmHash,
  salt: randomBytes(32),
  constructorArgs: [new Address(signer.publicKey()).toScVal()],
}));
if (!deployment.returnValue) throw new Error("Deployment returned no contract ID.");
const contractId = Address.fromScVal(deployment.returnValue).toString();

const adminRead = await submit(new Contract(contractId).call("get_admin"));
if (!adminRead.returnValue) throw new Error("get_admin returned no value.");
const observedAdmin = Address.fromScVal(adminRead.returnValue).toString();
if (observedAdmin !== signer.publicKey()) throw new Error("Constructor admin mismatch.");

const upgrade = await submit(new Contract(contractId).call(
  "upgrade",
  nativeToScVal(wasmHash, { type: "bytes" }),
));

process.stdout.write(`${JSON.stringify({
  network: "testnet",
  rpc: RPC_URL,
  horizon: HORIZON_URL,
  sourceAndAdmin: signer.publicKey(),
  wasmSha256: wasmHash.toString("hex"),
  uploadTransaction: upload.txHash,
  contractId,
  deploymentTransaction: deployment.txHash,
  adminReadTransaction: adminRead.txHash,
  selfUpgradeTransaction: upgrade.txHash,
}, null, 2)}\n`);
