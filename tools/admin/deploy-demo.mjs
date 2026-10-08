// Testnet-only end-to-end deployment and immutable replacement from an exact GitHub build.
import assert from "node:assert/strict";
import { randomBytes } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";
import { parseArgs } from "node:util";
import { Keypair, Address } from "@stellar/stellar-sdk";
import { githubBuild } from "./github-build.mjs";
import { planDeployment } from "./deploy.mjs";
import { eligibility, prepare, state, validate } from "./transfer.mjs";
import { actionValue, address, build, bytes, connect, Contract, Operation, read, rpc, scValToNative, wait } from "./rpc.mjs";
const { values: v } = parseArgs({ options: Object.fromEntries(["build-run", "build-commit", "out"].map((key) => [key, { type: "string" }])) });
if (!v.out) throw new Error("--out required");
const artifact = await githubBuild(v["build-run"], v["build-commit"]);
const signer = Keypair.random(); // Disposable Testnet key stays in memory.
const source = signer.publicKey();
const funded = await fetch(`https://friendbot.stellar.org/?addr=${source}`); assert(funded.ok);
const ctx = await connect("testnet");
const transactions = [];
await mkdir(v.out, { recursive: true });
const save = async () => writeFile(`${v.out}/receipt.json`, `${JSON.stringify({ network: "testnet", build: artifact.build, source, transactions }, null, 2)}\n`, { mode: 0o600 });
async function send(label, tx) {
  tx.sign(signer);
  const hash = Buffer.from(tx.hash()).toString("hex");
  const record = { label, hash, status: "SUBMISSION_PENDING", url: `https://stellar.expert/explorer/testnet/tx/${hash}` };
  transactions.push(record); await save();
  const result = await ctx.server.sendTransaction(tx); assert.notEqual(result.status, "ERROR", label);
  const confirmed = await wait(ctx, hash); record.status = confirmed.status; record.ledger = confirmed.ledger; await save();
  assert.equal(confirmed.status, "SUCCESS", label); console.log(`${label}: ${record.url}`); return confirmed;
}
async function call(label, contract, method, ...args) {
  return send(label, await ctx.server.prepareTransaction(await build(ctx, source, new Contract(contract).call(method, ...args))));
}
await send("upload verified Hello World", await ctx.server.prepareTransaction(await build(ctx, source, Operation.uploadContractWasm({ wasm: artifact.files.hello }))));
const targetTx = await ctx.server.prepareTransaction(await build(ctx, source, Operation.createCustomContract({ address: new Address(source), wasmHash: Buffer.from(artifact.build.hashes.hello, "hex"), salt: randomBytes(32), constructorArgs: [address(source)] })));
const target = Address.fromScVal((await send("deploy verified Hello World", targetTx)).returnValue).toString();
async function fresh(label) {
  const ui = await planDeployment({ network: "testnet", source, governance: source, target, delayLedgers: 30, maxFeeStroops: "5000000000", salt: randomBytes(32).toString("hex") }, artifact.build, artifact.files.timelock);
  for (const step of ["upload-helper", "deploy-helper"]) {
    if (eligibility(ui.m, await state(ui), step).done) continue;
    const tx = await prepare(ui, step); validate(ui, tx, step); await send(`${label}: ${step}`, tx);
  }
  return ui;
}
const original = await fresh("initial helper");
for (const step of ["propose-admin", "accept-management"]) await send(step, await prepare(original, step));
assert((await state(original)).adopted);
const replacement = await fresh("replacement helper");
assert.equal(replacement.m.initialAdmin, original.m.helper);
assert(!eligibility(replacement.m, await state(replacement), "propose-admin").ready);
const action = actionValue(["TransferAdmin", replacement.m.helper]);
const predecessor = bytes(Buffer.alloc(32)), salt = bytes(randomBytes(32));
const scheduled = await call("schedule replacement", original.m.helper, "schedule", action, predecessor, salt);
const id = scheduled.returnValue;
const ready = scValToNative(await read(ctx, source, original.m.helper, "ready_ledger", id));
for (const [label, method, code] of [["duplicate schedule", "schedule", 4000], ["early replacement", "execute", 4002]]) {
  const simulation = await ctx.server.simulateTransaction(await build(ctx, source, new Contract(original.m.helper).call(method, action, predecessor, salt)));
  assert(rpc.Api.isSimulationError(simulation)); assert(simulation.error.includes(`Error(Contract, #${code})`), simulation.error);
  console.log(`${label}: rejected with ${code}`);
}
console.log(`Replacement waits until ledger ${ready}`);
while ((await ctx.server.getLatestLedger()).sequence < ready) await new Promise((resolve) => setTimeout(resolve, 10000));
await call("post-delay replacement nomination", original.m.helper, "execute", action, predecessor, salt);
assert.equal((await state(replacement)).owner, original.m.helper);
await send("fresh helper accepts management", await prepare(replacement, "accept-management"));
const final = await state(replacement); assert(final.adopted); assert.equal(final.owner, replacement.m.helper);
const oldPause = await ctx.server.simulateTransaction(await build(ctx, source, new Contract(original.m.helper).call("pause_now")));
assert(rpc.Api.isSimulationError(oldPause)); assert(oldPause.error.includes("Error(Contract, #102)"));
await writeFile(`${v.out}/deployment.json`, `${JSON.stringify(replacement.m, null, 2)}\n`, { mode: 0o600 });
await writeFile(`${v.out}/result.json`, `${JSON.stringify({ outcome: "passed", network: "testnet", build: artifact.build, target, original: original.m, replacement: replacement.m, readyLedger: ready, final, transactions, checks: ["exact GitHub build", "browser-core upload/deploy validation", "constructor uses explicit final governance", "initial adoption", "old contract captured as administrator", "duplicate schedule rejected 4000 simulation", "early replacement rejected 4002 simulation", "post-delay nomination", "fresh helper acceptance", "retired helper cannot pause 102 simulation"] }, null, 2)}\n`, { mode: 0o600 });
console.log("PASSED verified-build deployment and immutable replacement");
