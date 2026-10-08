// Resumable standalone proof. Signing stays in Stellar CLI secure storage.
import assert from "node:assert/strict";
import { randomBytes } from "node:crypto";
import { spawn } from "node:child_process";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import { parseArgs } from "node:util";
import { Address, SorobanDataBuilder } from "@stellar/stellar-sdk";
import { actionValue, address, build, bytes, connect, Contract, Operation, read, rpc, scValToNative, TransactionBuilder, u32, wait, wasm } from "./rpc.mjs";
import { latestCloseTime } from "./transfer.mjs";
import { githubBuild } from "./github-build.mjs";
import { HELPER_WASM_HASH } from "./release.mjs";
import { proofFeeCap } from "./proof-fees.mjs";

const { positionals: [command], values: v } = parseArgs({ allowPositionals: true, options: Object.fromEntries(
  ["network", "identity", "stellar", "out", "delay", "fee", "testnet-max-fee-stroops", "diagnostic-probe", "build-run", "build-commit"].map((key) => [key, { type: "string" }]),
) });
if (!["start", "finish"].includes(command) || !v.identity || !v.out) throw new Error("Use start|finish --network mainnet|testnet --identity <secure-store alias> --out <private run directory>");
if (!/^[a-z][a-z0-9-]{1,63}$/.test(v.identity)) throw new Error("Use a secure-store identity alias, never a secret key");
const maxFee = proofFeeCap(v.network, v["testnet-max-fee-stroops"]);
const ctx = await connect(v.network);
if (v.network === "mainnet" && command === "start") {
  if (!/^[0-9a-f]{64}$/.test(v["diagnostic-probe"] ?? "")) throw new Error("Mainnet requires --diagnostic-probe <existing failed Soroban transaction hash> before deployment");
  const probe = await ctx.server.getTransaction(v["diagnostic-probe"]);
  if (probe.status !== "FAILED" || !probe.diagnosticEventsXdr?.length) throw new Error("Mainnet RPC diagnostic availability is not verified; no deployment will be submitted");
}
ctx.inclusionFee = v.fee ?? "10000";
if (!/^\d+$/.test(ctx.inclusionFee) || BigInt(ctx.inclusionFee) < 100n) throw new Error("Invalid inclusion fee");
const cli = v.stellar ?? "stellar";
const cliEnv = { ...process.env };
for (const key of ["STELLAR_SIGN_WITH_KEY", "STELLAR_SIGN_WITH_LEDGER", "STELLAR_SIGN_WITH_LAB", "STELLAR_CONFIG_DIR"]) delete cliEnv[key];
async function cliRun(args, input = "") {
  return new Promise((resolve, reject) => {
    const child = spawn(cli, args, { env: cliEnv, stdio: ["pipe", "pipe", "pipe"] });
    let output = "";
    child.stdout.on("data", (data) => { output += data; });
    // Do not print credential-store diagnostics; report only the exit status.
    child.stderr.resume(); child.on("error", reject);
    child.on("close", (code) => code === 0 ? resolve(output.trim()) : reject(new Error(`Stellar CLI failed (${code})`)));
    child.stdin.end(input);
  });
}
const source = await cliRun(["keys", "public-key", v.identity]);
await mkdir(v.out, { recursive: true, mode: 0o700 });
let report;
try { report = JSON.parse(await readFile(`${v.out}/receipt.json`, "utf8")); }
catch (error) { if (error.code !== "ENOENT") throw error; }
if (!report) {
  if (command !== "start") throw new Error("No started proof");
  const delay = v.network === "mainnet" ? 17280 : Number(v.delay ?? 30);
  if (!Number.isSafeInteger(delay) || delay < 1) throw new Error("Invalid delay");
  if (v.network === "mainnet" && v.delay && Number(v.delay) !== 17280) throw new Error("Mainnet proof must use 17,280 ledgers");
  report = { version: 2, network: v.network, source, identity: v.identity, delayLedgers: delay, startedAt: new Date().toISOString(), transactions: [], checks: [], salts: Object.fromEntries(["hello", "timelock", "upgrade", "unpause"].map((name) => [name, randomBytes(32).toString("hex")])) };
}
assert.equal(report.version, 2, "Previous factory proof receipts use a different ABI; retain them and choose a new proof directory");
assert.equal(report.network, v.network); assert.equal(report.source, source); assert.equal(report.identity, v.identity);
const save = async () => {
  await writeFile(`${v.out}/receipt.tmp`, `${JSON.stringify(report, null, 2)}\n`, { mode: 0o600 });
  await rename(`${v.out}/receipt.tmp`, `${v.out}/receipt.json`);
};
await save();
const horizon = v.network === "mainnet" ? "https://horizon.stellar.org" : "https://horizon-testnet.stellar.org";
const response = await fetch(`${horizon}/accounts/${source}`);
if (!response.ok) throw new Error(`Proof signer ${source} is not funded on ${v.network}`);
const policy = await response.json();
assert.equal(policy.signers.length, 1, "Proof requires a separate single-signer account");
assert.equal(policy.signers[0].key, source);
assert(policy.signers[0].weight > 0 && Object.values(policy.thresholds).every((threshold) => threshold <= policy.signers[0].weight));
const readNative = async (id, method, ...args) => scValToNative(await read(ctx, source, id, method, ...args));
const zero = bytes(Buffer.alloc(32)); const salt = (name) => bytes(Buffer.from(report.salts[name], "hex"));
async function send(label, operation, { hint, expected = "SUCCESS", guard } = {}) {
  const existing = report.transactions.find((tx) => tx.label === label);
  if (existing) {
    const result = await ctx.server.getTransaction(existing.hash);
    if (!["SUCCESS", "FAILED"].includes(result.status)) {
      const original = TransactionBuilder.fromXDR(existing.envelopeXdr, ctx.passphrase);
      const expired = await latestCloseTime({ m: { network: v.network } }) > Number(original.timeBounds.maxTime);
      const sequence = BigInt((await ctx.server.getAccount(source)).sequenceNumber());
      if (result.status === "NOT_FOUND" && expired && sequence < BigInt(original.sequence)) {
        if (guard) await guard();
        existing.status = "EXPIRED_UNSUBMITTED";
        report.transactions.splice(report.transactions.indexOf(existing), 1);
        report.expiredTransactions ??= []; report.expiredTransactions.push(existing); await save();
        return send(label, operation, { hint, expected, guard }); // expired body provably never consumed the source sequence
      }
      throw new Error(`Uncertain or archived ${existing.hash}; source sequence may have been consumed. Verify the historical receipt before any retry.`);
    }
    assert.equal(result.status, expected, label);
    existing.status = result.status; existing.ledger = result.ledger;
    if (result.returnValue) existing.returnValue = result.returnValue.toXDR("base64");
    await save(); return { record: existing, result };
  }
  if (guard) await guard();
  let transaction = await build(ctx, source, operation);
  if (hint) transaction = new TransactionBuilder(await ctx.server.getAccount(source), { fee: ctx.inclusionFee, networkPassphrase: ctx.passphrase })
    .addOperation(operation).setSorobanData(hint).setTimeout(1800).build();
  else transaction = await ctx.server.prepareTransaction(transaction);
  if (BigInt(transaction.fee) > maxFee) throw new Error("Proof transaction exceeds selected fee cap; stop and requote");
  const signed = await cliRun(["tx", "sign", "--network-passphrase", ctx.passphrase, "--sign-with-key", v.identity], transaction.toXDR());
  const signedTx = TransactionBuilder.fromXDR(signed, ctx.passphrase);
  assert(Buffer.from(signedTx.hash()).equals(Buffer.from(transaction.hash())), "Signing must preserve the body");
  const hash = Buffer.from(transaction.hash()).toString("hex");
  const record = { label, hash, url: `https://stellar.expert/explorer/${v.network === "mainnet" ? "public" : "testnet"}/tx/${hash}`, status: "SUBMISSION_PENDING", envelopeXdr: transaction.toXDR(), feeStroops: transaction.fee };
  report.transactions.push(record); await save(); // journal BEFORE network submission
  const sent = await ctx.server.sendTransaction(signedTx);
  if (sent.status === "ERROR") { record.status = "REJECTED"; await save(); throw new Error(`Rejected ${hash}; inspect before any new attempt`); }
  const result = await wait(ctx, hash);
  record.status = result.status; record.ledger = result.ledger;
  record.resultXdr = result.resultXdr.toXDR("base64");
  if (result.returnValue) record.returnValue = result.returnValue.toXDR("base64");
  record.diagnosticEvents = result.diagnosticEventsXdr?.map((event) => event.toXDR("base64")) ?? [];
  await save(); console.log(`${label}: ${record.status} ${record.url}`);
  assert.equal(result.status, expected, label);
  return { record, result };
}
async function reject(label, operation, code) {
  if (report.checks.some((check) => check.label === label && check.contractError === code)) return;
  const simulation = await ctx.server.simulateTransaction(await build(ctx, source, operation));
  assert(rpc.Api.isSimulationError(simulation), label);
  assert(simulation.error.includes(`Error(Contract, #${code})`), simulation.error);
  report.checks.push({ label, contractError: code, ledger: simulation.latestLedger, error: simulation.error }); await save();
}
async function deploy(label, hash, args, s) {
  const { result } = await send(label, Operation.createCustomContract({ address: new Address(source), wasmHash: hash, salt: Buffer.from(report.salts[s], "hex"), constructorArgs: args }));
  return Address.fromScVal(result.returnValue).toString();
}
if (command === "start") {
  const code = {};
  if (v.network === "mainnet" || v["build-run"]) {
    const artifact = await githubBuild(v["build-run"], v["build-commit"]);
    if (report.build) assert.deepEqual(report.build, artifact.build, "Cannot change selected build on resume");
    report.build = artifact.build;
    for (const [name, value] of Object.entries(artifact.files)) code[name] = { value, hash: Buffer.from(artifact.build.hashes[name], "hex") };
  } else for (const name of ["timelock", "hello"]) code[name] = await wasm(name);
  const hashes = Object.fromEntries(Object.entries(code).map(([name, file]) => [name, file.hash.toString("hex")]));
  if (report.wasmHashes) assert.deepEqual(hashes, report.wasmHashes, "Cannot change proof artifacts on resume");
  if (v.network === "mainnet" && hashes.timelock !== HELPER_WASM_HASH) throw new Error("Mainnet helper differs from reviewed build");
  report.wasmHashes = hashes; await save();
  for (const [name, file] of Object.entries(code)) await send(`upload ${name}`, Operation.uploadContractWasm({ wasm: file.value }));
  report.target = await deploy("deploy Hello World", code.hello.hash, [address(source)], "hello"); await save();
  report.helper = await deploy("deploy timelock controller", code.timelock.hash, [address(report.target), address(source), u32(report.delayLedgers)], "timelock"); await save();
  await send("nominate timelock controller", new Contract(report.target).call("propose_admin", address(report.helper)));
  await send("accept management", new Contract(report.helper).call("accept_management"));
  assert.equal(await readNative(report.target, "get_admin"), report.helper);
  assert.equal((await readNative(report.helper, "get_config")).governance, source);
  const helper = new Contract(report.helper);
  const upgrade = actionValue(["Upgrade", hashes.hello]);
  const scheduled = await send("schedule upgrade", helper.call("schedule", upgrade, zero, salt("upgrade")));
  report.upgradeId = Buffer.from(scValToNative(scheduled.result.returnValue)).toString("hex");
  report.upgradeReady = await readNative(report.helper, "ready_ledger", bytes(Buffer.from(report.upgradeId, "hex"))); await save();
  await reject("early upgrade", helper.call("execute", upgrade, zero, salt("upgrade")), 4002);
  await reject("duplicate schedule", helper.call("schedule", upgrade, zero, salt("upgrade")), 4000);
  const tx = TransactionBuilder.fromXDR(scheduled.record.envelopeXdr, ctx.passphrase);
  const data = new SorobanDataBuilder(tx.toEnvelope().v1().tx().ext().value())
    .setResources(20_000_000, 100_000, 20_000).setResourceFee("10000000").build();
  const denied = await send("on-chain early rejection", helper.call("execute", upgrade, zero, salt("upgrade")), { hint: data, expected: "FAILED", guard: async () => {
    if ((await ctx.server.getLatestLedger()).sequence + 5 >= report.upgradeReady) throw new Error("Early-proof window elapsed; do not execute this expected-failure transaction");
  } });
  assert(denied.result.ledger < report.upgradeReady);
  assert(denied.result.diagnosticEventsXdr?.some((event) => event.event().body().v0().topics().some((topic) => topic.switch().name === "scvError" && topic.error().switch().name === "sceContract" && topic.error().contractCode() === 4002)), "Failure must specifically prove timelock rejection");
  await send("immediate pause", helper.call("pause_now"));
  assert.equal(await readNative(report.target, "is_paused"), true);
  const resume = await send("schedule unpause", helper.call("schedule", actionValue(["Unpause"]), bytes(Buffer.from(report.upgradeId, "hex")), salt("unpause")));
  report.unpauseId = Buffer.from(scValToNative(resume.result.returnValue)).toString("hex");
  report.unpauseReady = await readNative(report.helper, "ready_ledger", bytes(Buffer.from(report.unpauseId, "hex")));
  await reject("early unpause", helper.call("execute", actionValue(["Unpause"]), bytes(Buffer.from(report.upgradeId, "hex")), salt("unpause")), 4002);
  const unpauseTx = TransactionBuilder.fromXDR(resume.record.envelopeXdr, ctx.passphrase);
  const unpauseData = new SorobanDataBuilder(unpauseTx.toEnvelope().v1().tx().ext().value())
    .setResources(20_000_000, 100_000, 20_000).setResourceFee("10000000").build();
  const deniedUnpause = await send("on-chain second early operation rejection", helper.call("execute", actionValue(["Unpause"]), bytes(Buffer.from(report.upgradeId, "hex")), salt("unpause")), { hint: unpauseData, expected: "FAILED", guard: async () => {
    if ((await ctx.server.getLatestLedger()).sequence + 5 >= report.unpauseReady) throw new Error("Second early-proof window elapsed; do not execute this expected-failure transaction");
  } });
  assert(deniedUnpause.result.ledger < report.unpauseReady);
  assert(deniedUnpause.result.diagnosticEventsXdr?.some((event) => event.event().body().v0().topics().some((topic) => topic.switch().name === "scvError" && topic.error().switch().name === "sceContract" && topic.error().contractCode() === 4002)), "Second operation must fail specifically because it is not ready");
  report.stage = "WAITING"; await save();
  console.log(`Waiting: upgrade ledger ${report.upgradeReady}; unpause ledger ${report.unpauseReady}. Run finish after both are ready.`);
} else {
  assert(report.upgradeReady && report.unpauseReady && report.helper, "Start phase is incomplete");
  const ledger = (await ctx.server.getLatestLedger()).sequence;
  if (ledger < Math.max(report.upgradeReady, report.unpauseReady)) {
    console.log(`WAITING: ledger ${ledger}; required ${Math.max(report.upgradeReady, report.unpauseReady)}`); process.exit(0);
  }
  const helper = new Contract(report.helper);
  const upgrade = actionValue(["Upgrade", report.wasmHashes.hello]);
  await send("post-delay upgrade", helper.call("execute", upgrade, zero, salt("upgrade")));
  assert.equal(await readNative(report.target, "hello"), "Hello World");
  assert.equal(await readNative(report.target, "value"), 42);
  await reject("completed upgrade replay", helper.call("execute", upgrade, zero, salt("upgrade")), 4002);
  await send("post-delay unpause", helper.call("execute", actionValue(["Unpause"]), bytes(Buffer.from(report.upgradeId, "hex")), salt("unpause")));
  assert.equal(await readNative(report.target, "is_paused"), false);
  assert.equal(await readNative(report.target, "get_admin"), report.helper);
  report.stage = "PASSED"; report.finishedAt = new Date().toISOString(); await save();
  console.log("PASSED: upgrade, unpause and replay protection");
}
