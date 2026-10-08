import { context as transferContext, manifest, prepare as prepareTransfer } from "./transfer.mjs";
import { githubBuild } from "./github-build.mjs";
import { parseArgs } from "node:util";
import { readFile, writeFile } from "node:fs/promises";
import { actionValue, address, Address, build, bytes, connect, Contract, Operation, read, scValToNative, TransactionBuilder, u32, wait, wasm } from "./rpc.mjs";

const { positionals, values: v } = parseArgs({ allowPositionals: true, options: Object.fromEntries(
  ["network", "source", "governance", "controller", "action", "salt", "predecessor", "id", "target", "delay", "out", "file", "signed", "fee", "successor", "build-run", "build-commit"].map((key) => [key, { type: "string" }]),
) });
const [command, method] = positionals;
const ctx = await connect(v.network);
if (v.fee && (!/^\d+$/.test(v.fee) || BigInt(v.fee) < 100n)) throw new Error("--fee must be at least 100 stroops");
ctx.inclusionFee = v.fee ?? "100";
async function helperBuild() {
  if (ctx.network === "mainnet" || v["build-run"]) {
    const artifact = await githubBuild(v["build-run"], v["build-commit"]);
    return { value: artifact.files.timelock, hash: Buffer.from(artifact.build.hashes.timelock, "hex"), provenance: artifact.build };
  }
  return wasm("timelock");
}
const buffer = (s) => {
  if (!/^[0-9a-f]{64}$/i.test(s ?? "")) throw new Error("Expected explicit 32-byte hex value");
  return bytes(Buffer.from(s, "hex"));
};

if (command === "inspect") {
  const config = scValToNative(await read(ctx, v.source, v.controller, "get_config"));
  const owner = scValToNative(await read(ctx, v.source, config.target, "get_admin"));
  console.log(JSON.stringify({ network: ctx.network, controller: v.controller, config, owner,
    ...(v.id ? { state: scValToNative(await read(ctx, v.source, v.controller, "state", buffer(v.id))), readyLedger: scValToNative(await read(ctx, v.source, v.controller, "ready_ledger", buffer(v.id))) } : {}),
  }, null, 2));
} else if (command === "prepare") {
  if (!v.out || !v.source) throw new Error("--source and --out are required");
  let operation;
  let config;
  let prepared;
  if (["propose-admin", "accept-management", "accept-governance"].includes(method)) {
    if (!v.file) throw new Error("--file deployment JSON required for sequential handoff");
    config = manifest(JSON.parse(await readFile(v.file, "utf8")), v.governance);
    if (config.network !== ctx.network) throw new Error("Deployment file network differs from the selected network");
    prepared = await prepareTransfer(await transferContext(config), method);
    if (prepared.source !== v.source) throw new Error("Source differs from the required handoff signer");
  } else if (method === "upload-helper") {
    const helper = await helperBuild();
    config = { helperHash: helper.hash.toString("hex") };
    operation = Operation.uploadContractWasm({ wasm: helper.value });
  } else if (method === "deploy-helper") {
    const delay = Number(v.delay);
    if (!Number.isSafeInteger(delay) || delay <= 0 || (ctx.network === "mainnet" && delay !== 17280)) throw new Error("Mainnet helper requires 17,280 ledgers; Testnet requires a positive delay");
    const initialAdmin = scValToNative(await read(ctx, v.source, v.target, "get_admin"));
    if (scValToNative(await read(ctx, v.source, v.target, "get_pending_admin")) !== null) throw new Error("Target already has a pending handoff");
    const helper = await helperBuild();
    config = { target: v.target, governance: v.source, initial_admin: initialAdmin, delay, helperHash: helper.hash.toString("hex") };
    operation = Operation.createCustomContract({ address: new Address(v.source), wasmHash: helper.hash, salt: Buffer.from(v.salt, "hex"), constructorArgs: [address(v.target), address(v.source), u32(delay)] });
    buffer(v.salt);
  } else if (method === "propose-governance") {
    config = scValToNative(await read(ctx, v.source, v.controller, "get_config"));
    if (config.governance !== v.source) throw new Error("Source must be the current bootstrap governance account");
    operation = new Contract(v.controller).call("propose_governance", address(v.successor));
  } else {
    config = scValToNative(await read(ctx, v.source, v.controller, "get_config"));
    const owner = scValToNative(await read(ctx, v.source, config.target, "get_admin"));
    if (owner !== v.controller) throw new Error("Controller is not the managed contract owner");
    if (["schedule", "cancel", "pause_now"].includes(method) && config.governance !== v.source) throw new Error("Source must be the configured governance account");
    const contract = new Contract(v.controller);
    if (["schedule", "execute"].includes(method)) operation = contract.call(method, actionValue(JSON.parse(v.action)), buffer(v.predecessor ?? "0".repeat(64)), buffer(v.salt));
    else if (method === "cancel") operation = contract.call(method, buffer(v.id));
    else if (method === "pause_now") operation = contract.call(method);
    else throw new Error("Unknown admin method");
  }
  const transaction = prepared ?? await ctx.server.prepareTransaction(await build(ctx, v.source, operation));
  const data = transaction.toEnvelope().v1().tx().ext().value();
  const packet = { network: ctx.network, source: v.source, controller: v.controller ?? null,
    config, method, action: v.action ? JSON.parse(v.action) : null, salt: v.salt ?? null, predecessor: v.predecessor ?? "0".repeat(64),
    feeStroops: transaction.fee, resourceFeeStroops: data.resourceFee().toString(), timeBounds: transaction.timeBounds, sequence: transaction.sequence,
    hash: Buffer.from(transaction.hash()).toString("hex"), envelopeXdr: transaction.toXDR(), signedEnvelopeXdr: null };
  await writeFile(v.out, `${JSON.stringify(packet, null, 2)}\n`, { mode: 0o600, flag: "wx" });
  console.log(JSON.stringify({ file: v.out, hash: packet.hash, config, method }, null, 2));
} else if (command === "attach") {
  if (!v.out || !v.signed) throw new Error("--signed XDR file and --out packet required");
  const packet = JSON.parse(await readFile(v.file, "utf8"));
  const signedEnvelopeXdr = (await readFile(v.signed, "utf8")).trim();
  const transaction = TransactionBuilder.fromXDR(signedEnvelopeXdr, ctx.passphrase);
  if (packet.network !== ctx.network || Buffer.from(transaction.hash()).toString("hex") !== packet.hash || transaction.source !== packet.source) throw new Error("Signed transaction body or network changed");
  await writeFile(v.out, `${JSON.stringify({ ...packet, signedEnvelopeXdr }, null, 2)}\n`, { mode: 0o600, flag: "wx" });
  console.log(JSON.stringify({ file: v.out, hash: packet.hash, signatures: transaction.signatures.length }));
} else if (command === "submit") {
  const packet = JSON.parse(await readFile(v.file, "utf8"));
  if (packet.network !== ctx.network || !packet.signedEnvelopeXdr) throw new Error("Signed packet and matching network required");
  const transaction = TransactionBuilder.fromXDR(packet.signedEnvelopeXdr, ctx.passphrase);
  if (Buffer.from(transaction.hash()).toString("hex") !== packet.hash || transaction.source !== packet.source) throw new Error("Signed transaction body changed");
  if (!transaction.signatures.length) throw new Error("No signatures attached");
  const sent = await ctx.server.sendTransaction(transaction);
  if (sent.status === "ERROR") throw new Error("RPC rejected transaction; do not retry unchanged");
  const result = (await wait(ctx, sent.hash)).status;
  console.log(JSON.stringify({ hash: sent.hash, result }));
  if (result !== "SUCCESS") throw new Error(`Transaction ${sent.hash} failed`);
} else throw new Error("Use inspect, prepare <schedule|execute|cancel|pause_now|upload-helper|deploy-helper|propose-governance|propose-admin|accept-management|accept-governance>, attach, or submit");
