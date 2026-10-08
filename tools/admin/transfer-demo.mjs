// Testnet-only exercise of the exact signing-page transaction core.
import assert from "node:assert/strict";
import { randomBytes } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";
import { parseArgs } from "node:util";
import { Keypair, Operation, Address } from "@stellar/stellar-sdk";
import { address, build, bytes, connect, Contract, scValToNative, u32, wait, wasm } from "./rpc.mjs";
import { account, context, eligibility, manifest, mergeSignatures, prepare, signatures, state } from "./transfer.mjs";
const { values: v } = parseArgs({ options: { out: { type: "string" } } });
if (!v.out) throw new Error("--out required");
await mkdir(v.out, { recursive: true, mode: 0o700 });
const ctx = await connect("testnet");
const deployer = Keypair.random(), old = Keypair.random(), co = Keypair.random(), third = Keypair.random(), recipient = Keypair.random();
await Promise.all([deployer, old, recipient].map(async (k) => {
  const response = await fetch(`https://friendbot.stellar.org/?addr=${k.publicKey()}`); assert(response.ok);
}));
const records = [];
async function send(label, transaction, keys) {
  keys.forEach((key) => transaction.sign(key));
  const sent = await ctx.server.sendTransaction(transaction); assert.notEqual(sent.status, "ERROR", label);
  const result = await wait(ctx, sent.hash); assert.equal(result.status, "SUCCESS", label);
  records.push({ label, hash: sent.hash, url: `https://stellar.expert/explorer/testnet/tx/${sent.hash}`, ledger: result.ledger });
  console.log(`${label}: ${records.at(-1).url}`); return result;
}
const { TransactionBuilder } = await import("@stellar/stellar-sdk");
let tx = new TransactionBuilder(await ctx.server.getAccount(old.publicKey()), { fee: "300", networkPassphrase: ctx.passphrase })
  .addOperation(Operation.setOptions({ signer: { ed25519PublicKey: co.publicKey(), weight: 1 } }))
  .addOperation(Operation.setOptions({ signer: { ed25519PublicKey: third.publicKey(), weight: 1 } }))
  .addOperation(Operation.setOptions({ masterWeight: 1, lowThreshold: 2, medThreshold: 2, highThreshold: 2 })).setTimeout(300).build();
await send("fixture 2-of-3 setup", tx, [old]);
const hello = await wasm("hello"), helper = await wasm("timelock");
async function deploy(label, hash, args) {
  const transaction = await ctx.server.prepareTransaction(await build(ctx, deployer.publicKey(), Operation.createCustomContract({ address: new Address(deployer.publicKey()), wasmHash: hash, salt: randomBytes(32), constructorArgs: args })));
  return Address.fromScVal((await send(label, transaction, [deployer])).returnValue).toString();
}
const target = await deploy("deploy transfer fixture", hello.hash, [address(old.publicKey())]);
const controller = await deploy("deploy bootstrap helper", helper.hash, [address(target), address(deployer.publicKey()), u32(30)]);
await send("deployer proposes recipient governance", await ctx.server.prepareTransaction(await build(ctx, deployer.publicKey(), new Contract(controller).call("propose_governance", address(recipient.publicKey())))), [deployer]);
const m = manifest({ version: 1, network: "testnet", target, helper: controller, initialAdmin: old.publicKey(), governance: recipient.publicKey(), delayLedgers: 30, helperWasmHash: helper.hash.toString("hex"), targetWasmHash: hello.hash.toString("hex"), maxFeeStroops: "100000000" });
const ui = await context(m);
for (const step of ["accept-governance", "propose-admin", "accept-management"]) {
  assert(eligibility(m, await state(ui), step).ready);
  tx = await prepare(ui, step);
  const policy = await account(ui, tx.source);
  if (step === "propose-admin") {
    tx.sign(old); assert.equal(signatures(tx, policy).weight, 1);
    const denied = await ui.server.sendTransaction(tx); assert.equal(denied.status, "ERROR"); assert.equal(denied.errorResult.result().switch().name, "txBadAuth");
    const imported = TransactionBuilder.fromXDR(tx.toXDR(), ui.passphrase); imported.sign(co);
    mergeSignatures(tx, imported, ui, step); assert.equal(signatures(tx, policy).weight, 2);
    await send(step, tx, []);
  } else {
    tx.sign(recipient); assert.equal(signatures(tx, policy).weight, 1);
    await send(step, tx, []);
  }
}
const final = await state(ui);
assert(final.adopted); assert.equal(final.owner, controller); assert.equal(final.config.governance, recipient.publicKey());
await writeFile(`${v.out}/deployment.json`, `${JSON.stringify(m, null, 2)}\n`, { mode: 0o600 });
await writeFile(`${v.out}/receipt.json`, `${JSON.stringify({ network: "testnet", outcome: "passed", deployment: m, final, transactions: records, checks: ["deployer-to-recipient governance handoff", "original 2-of-3 nomination rejects one signer", "same-body import merges two signatures", "recipient accepts management"] }, null, 2)}\n`, { mode: 0o600 });
console.log("PASSED signing-page transaction core");
