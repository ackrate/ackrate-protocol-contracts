import { Buffer } from "buffer";
import { Address, Contract, Keypair, Networks, Operation, hash, Transaction, TransactionBuilder, rpc, scValToNative, xdr } from "@stellar/stellar-sdk";
import { HELPER_WASM_HASH, PRODUCTION_GOVERNANCE, REVIEWED_HELPER_HASHES, V2_WASM_HASH } from "./release.mjs";

export const V2 = "CCLZEBJXG4YVJEPBCR5F27N733BCK5HQJWZZGB3K54JVODY3VAGP4HWR";
export const V2_ADMIN = "GCIURCX7JHEKQLRTW6RDZU7OJUVCDM7WWNQPIKRERIHQOHSLW7UY7TXG";
export function manifest(value, expectedGovernance = null) {
  if (value.version !== 1 || !["testnet", "mainnet"].includes(value.network)) throw new Error("Invalid deployment file version/network");
  for (const field of ["target", "helper"]) if (!value[field]?.startsWith("C") || !(new Address(value[field]))) throw new Error(`Invalid ${field}`);
  new Address(value.initialAdmin);
  Keypair.fromPublicKey(value.governance);
  if (value.deployment) {
    Keypair.fromPublicKey(value.deployment.source);
    if (!/^[0-9a-f]{64}$/.test(value.deployment.salt ?? "")) throw new Error("Invalid deployment salt");
  }
  if (value.initialAdmin.startsWith("C") && value.network === "mainnet" && !REVIEWED_HELPER_HASHES.includes(value.previousHelperWasmHash)) throw new Error("Previous helper code needs a separately reviewed historical hash");
  for (const field of ["helperWasmHash", "targetWasmHash"]) if (!/^[0-9a-f]{64}$/.test(value[field] ?? "")) throw new Error(`Invalid ${field}`);
  if (!Number.isInteger(value.delayLedgers) || value.delayLedgers <= 0) throw new Error("Invalid delay");
  if (value.network === "mainnet" && (value.target !== V2 || (value.initialAdmin.startsWith("G") && value.initialAdmin !== V2_ADMIN) || value.delayLedgers !== 17280)) throw new Error("Mainnet deployment must match V2, its current owner and 17,280-ledger delay");
  if (value.network === "mainnet" && (value.helperWasmHash !== HELPER_WASM_HASH || value.targetWasmHash !== V2_WASM_HASH)) throw new Error("Mainnet deployment differs from the reviewed WASM pins");
  if (value.network === "mainnet") {
    const expected = PRODUCTION_GOVERNANCE ?? expectedGovernance;
    if (!expected) throw new Error("Enter expected recipient governance separately before importing or resuming Mainnet");
    if (value.governance !== expected) throw new Error("Entered expected governance differs from the deployment file. Verify the recipient before signing.");
  }
  if (!/^\d+$/.test(value.maxFeeStroops ?? "") || BigInt(value.maxFeeStroops) > 5000000000n || BigInt(value.maxFeeStroops) < 10000n) throw new Error("Fee cap must be between 0.001 and 500 XLM");
  return Object.freeze({ ...value });
}
export async function context(m) {
  const passphrase = m.network === "mainnet" ? Networks.PUBLIC : Networks.TESTNET;
  const server = new rpc.Server(m.network === "mainnet" ? "https://mainnet.sorobanrpc.com" : "https://soroban-testnet.stellar.org");
  if ((await server.getNetwork()).passphrase !== passphrase) throw new Error("RPC network mismatch");
  return { m, server, passphrase };
}
async function base(ctx, source, operation) {
  return new TransactionBuilder(await ctx.server.getAccount(source), { fee: "10000", networkPassphrase: ctx.passphrase }).addOperation(operation).setTimeout(1800).build();
}
export async function read(ctx, target, method) {
  const result = await ctx.server.simulateTransaction(await base(ctx, ctx.m.governance, new Contract(target).call(method)));
  if (!rpc.Api.isSimulationSuccess(result)) throw new Error(`Cannot read ${method}: ${result.error ?? "missing result"}`);
  return scValToNative(result.result.retval);
}
export async function codeHash(ctx, contract, optional = false) {
  const key = xdr.LedgerKey.contractData(new xdr.LedgerKeyContractData({ contract: new Address(contract).toScAddress(), key: xdr.ScVal.scvLedgerKeyContractInstance(), durability: xdr.ContractDataDurability.persistent() }));
  const result = await ctx.server.getLedgerEntries(key);
  if (!result.entries.length) { if (optional) return null; throw new Error(`Contract ${contract} unavailable`); }
  return Buffer.from(result.entries[0].val.contractData().val().instance().executable().wasmHash()).toString("hex");
}
export async function account(ctx, source) {
  const url = ctx.m.network === "mainnet" ? "https://horizon.stellar.org" : "https://horizon-testnet.stellar.org";
  const response = await fetch(`${url}/accounts/${source}`);
  if (!response.ok) throw new Error(`Account ${source} is unavailable or unfunded`);
  const value = await response.json();
  if (value.signers.some((s) => s.type !== "ed25519_public_key")) throw new Error("Only native Ed25519 signing policies are supported");
  return value;
}
export async function latestCloseTime(ctx) {
  const url = ctx.m.network === "mainnet" ? "https://horizon.stellar.org" : "https://horizon-testnet.stellar.org";
  const response = await fetch(`${url}/ledgers?order=desc&limit=1`);
  if (!response.ok) throw new Error("Cannot verify latest ledger close time");
  const ledger = (await response.json())._embedded.records[0];
  return Math.floor(Date.parse(ledger.closed_at) / 1000);
}
export async function codeInstalled(ctx) {
  const key = xdr.LedgerKey.contractCode(new xdr.LedgerKeyContractCode({ hash: Buffer.from(ctx.m.helperWasmHash, "hex") }));
  return (await ctx.server.getLedgerEntries(key)).entries.length > 0;
}
export async function state(ctx) {
  const m = ctx.m;
  const [helperHash, targetHash, owner, pendingAdmin, installed] = await Promise.all([
    codeHash(ctx, m.helper, Boolean(m.deployment)), codeHash(ctx, m.target), read(ctx, m.target, "get_admin"), read(ctx, m.target, "get_pending_admin"), codeInstalled(ctx),
  ]);
  if (targetHash !== m.targetWasmHash || (helperHash && helperHash !== m.helperWasmHash)) throw new Error("Deployed WASM differs from deployment file");
  if (m.initialAdmin.startsWith("G")) {
    const oldAccount = await account(ctx, m.initialAdmin);
    if (m.network === "mainnet" && ([V2_ADMIN, "GD3UEYYZRU53VBAVGEKR6HYQ3USQ3FEBT5BLOYEX356EFOM5SR5774GW", "GD57LQEI6PLLDWT5TVKUYNTKHRPEZEGRYK7OCJUSUP767PE2P7MXE73B"].some((key) => !oldAccount.signers.some((s) => s.key === key)) || oldAccount.signers.length !== 3 || oldAccount.signers.some((s) => s.weight !== 1) || Object.values(oldAccount.thresholds).some((n) => n !== 2))) throw new Error("V2 owner is no longer the reviewed 2-of-3 account");
  } else if (owner === m.initialAdmin) {
    const [oldHash, oldConfig, oldAdopted] = await Promise.all([codeHash(ctx, m.initialAdmin), read(ctx, m.initialAdmin, "get_config"), read(ctx, m.initialAdmin, "is_adopted")]);
    if ((m.network === "mainnet" ? !REVIEWED_HELPER_HASHES.includes(oldHash) || oldHash !== m.previousHelperWasmHash : oldHash !== (m.previousHelperWasmHash ?? m.helperWasmHash)) || oldConfig.target !== m.target || !oldAdopted || (m.network === "mainnet" && oldConfig.delay !== 17280)) throw new Error("Previous administrator is not the reviewed timelock managing this target");
  }
  if (!helperHash) return { owner, pendingAdmin, installed, deployed: false, adopted: false, config: null, pendingGovernance: null };
  const [config, pendingGovernance, adopted] = await Promise.all([read(ctx, m.helper, "get_config"), read(ctx, m.helper, "get_pending_governance"), read(ctx, m.helper, "is_adopted")]);
  if (config.target !== m.target || config.initial_admin !== m.initialAdmin || config.delay !== m.delayLedgers) throw new Error("Timelock configuration differs from deployment file");
  if (m.deployment && config.governance !== m.governance) throw new Error("Deployed governance differs from signed constructor");
  return { config, owner, pendingAdmin, pendingGovernance, adopted, installed, deployed: true };
}
export function deploymentOperation(ctx, step) {
  if (!ctx.m.deployment || !ctx.wasm || Buffer.from(hash(ctx.wasm)).toString("hex") !== ctx.m.helperWasmHash) throw new Error("Verified deployment preparation required");
  if (step === "upload-helper") return Operation.uploadContractWasm({ wasm: ctx.wasm });
  if (step === "deploy-helper") return Operation.createCustomContract({ address: new Address(ctx.m.deployment.source), wasmHash: Buffer.from(ctx.m.helperWasmHash, "hex"), salt: Buffer.from(ctx.m.deployment.salt, "hex"), constructorArgs: [new Address(ctx.m.target).toScVal(), new Address(ctx.m.governance).toScVal(), xdr.ScVal.scvU32(ctx.m.delayLedgers)] });
  throw new Error("Unknown deployment step");
}

export function specification(m, step) {
  if (["upload-helper", "deploy-helper"].includes(step)) {
    if (!m.deployment) throw new Error("Deployment parameters missing");
    return { source: m.deployment.source, contract: step === "deploy-helper" ? m.helper : m.helperWasmHash, method: step === "deploy-helper" ? "create_contract" : "upload_contract_wasm", args: [] };
  }
  if (step === "accept-governance") return { source: m.governance, contract: m.helper, method: "accept_governance", args: [] };
  if (step === "propose-admin") return { source: m.initialAdmin, contract: m.target, method: "propose_admin", args: [new Address(m.helper).toScVal()] };
  if (step === "accept-management") return { source: m.governance, contract: m.helper, method: "accept_management", args: [] };
  throw new Error("Unknown transfer step");
}
export function eligibility(m, s, step) {
  if (step === "upload-helper") return { done: s.installed, ready: Boolean(m.deployment) && !s.installed && !s.deployed && s.owner === m.initialAdmin && s.pendingAdmin === null };
  if (step === "deploy-helper") return { done: s.deployed, ready: Boolean(m.deployment) && s.installed && !s.deployed && s.owner === m.initialAdmin && s.pendingAdmin === null };
  if (s.deployed === false) return { done: false, ready: false };
  const governanceReady = s.config.governance === m.governance && s.pendingGovernance === null;
  if (step === "accept-governance") return { done: governanceReady, ready: !s.adopted && s.owner === m.initialAdmin && s.pendingAdmin === null && s.pendingGovernance === m.governance };
  if (step === "propose-admin") return { done: s.pendingAdmin === m.helper || s.owner === m.helper, ready: m.initialAdmin.startsWith("G") && governanceReady && !s.adopted && s.owner === m.initialAdmin && s.pendingAdmin === null };
  return { done: s.adopted && s.owner === m.helper, ready: governanceReady && !s.adopted && s.owner === m.initialAdmin && s.pendingAdmin === m.helper };
}
export async function prepare(ctx, step) {
  const s = await state(ctx);
  if (!eligibility(ctx.m, s, step).ready) throw new Error("This step is not ready in current chain state");
  const spec = specification(ctx.m, step);
  const tx = await ctx.server.prepareTransaction(await base(ctx, spec.source, (["upload-helper", "deploy-helper"].includes(step) ? deploymentOperation(ctx, step) : new Contract(spec.contract).call(spec.method, ...spec.args))));
  validate(ctx, tx, step); return tx;
}
export function identifyStep(ctx, tx, candidates) {
  if (!(tx instanceof Transaction) || tx.operations.length !== 1 || tx.operations[0].type !== "invokeHostFunction") return null;
  for (const step of candidates) {
    try {
      const spec = specification(ctx.m, step);
      const operation = ["upload-helper", "deploy-helper"].includes(step) ? deploymentOperation(ctx, step) : new Contract(spec.contract).call(spec.method, ...spec.args);
      if (tx.source === spec.source && Buffer.from(tx.operations[0].func.toXDR()).equals(Buffer.from(operation.body().invokeHostFunctionOp().hostFunction().toXDR()))) return step;
    } catch { /* This context has no deployment operation for this candidate. */ }
  }
  return null;
}
export function validate(ctx, tx, step, pinnedHash = null) {
  const spec = specification(ctx.m, step);
  if (!(tx instanceof Transaction) || tx.source !== spec.source || tx.operations.length !== 1 || tx.memo.type !== "none") throw new Error("Envelope source/operations/memo differs from transfer");
  if (pinnedHash && Buffer.from(tx.hash()).toString("hex") !== pinnedHash) throw new Error("Envelope body changed; signatures must cover the same transaction");
  if (BigInt(tx.fee) > BigInt(ctx.m.maxFeeStroops)) throw new Error(`Quoted maximum fee ${(Number(tx.fee) / 1e7).toFixed(7)} XLM exceeds your ${(Number(ctx.m.maxFeeStroops) / 1e7).toFixed(7)} XLM cap. Adjust the cap before preparing this operation again.`);
  const bounds = tx.timeBounds;
  const now = Math.floor(Date.now() / 1000);
  if (!bounds || Number(bounds.minTime) > now || Number(bounds.maxTime) <= now || Number(bounds.maxTime) > now + 1800) throw new Error("Envelope expired or has invalid time bounds");
  const envelopeTx = tx.toEnvelope().v1().tx();
  if (envelopeTx.cond().switch().name !== "precondTime") throw new Error("Unexpected transaction preconditions");
  const op = tx.operations[0];
  if (op.type !== "invokeHostFunction" || (op.source && op.source !== spec.source)) throw new Error("Unexpected host function");
  const deploying = ["upload-helper", "deploy-helper"].includes(step);
  const expected = (deploying ? deploymentOperation(ctx, step) : new Contract(spec.contract).call(spec.method, ...spec.args)).body().invokeHostFunctionOp().hostFunction();
  if (!Buffer.from(op.func.toXDR()).equals(Buffer.from(expected.toXDR()))) throw new Error("Contract call or arguments changed");
  if (step === "upload-helper") {
    if (op.auth.length !== 0) throw new Error("Upload must not authorize a contract call");
    return spec;
  }
  if (op.auth.length !== 1) throw new Error("Expected exactly one source-account authorization");
  const entry = op.auth[0];
  if (entry.credentials().switch().name !== "sorobanCredentialsSourceAccount") throw new Error("Unexpected external authorization");
  const root = entry.rootInvocation();
  const expectedKind = step === "deploy-helper" ? "sorobanAuthorizedFunctionTypeCreateContractV2HostFn" : "sorobanAuthorizedFunctionTypeContractFn";
  if (root.function().switch().name !== expectedKind || root.subInvocations().length !== 0 || !Buffer.from(root.function().value().toXDR()).equals(Buffer.from(op.func.value().toXDR()))) throw new Error("Authorization tree differs from transfer call");
  return spec;
}
export function signatures(tx, policy) {
  const seen = new Set();
  for (const signature of tx.signatures) {
    const signer = policy.signers.find((s) => {
      const key = Keypair.fromPublicKey(s.key);
      return Buffer.from(key.signatureHint()).equals(Buffer.from(signature.hint())) && key.verify(tx.hash(), signature.signature());
    });
    if (!signer) throw new Error("Envelope includes an invalid or unrelated signature");
    seen.add(signer.key);
  }
  const weight = policy.signers.filter((s) => seen.has(s.key)).reduce((n, s) => n + s.weight, 0);
  return { weight, required: Math.max(1, policy.thresholds.med_threshold), signers: [...seen] };
}
export function mergeSignatures(original, imported, ctx, step) {
  validate(ctx, imported, step, Buffer.from(original.hash()).toString("hex"));
  for (const sig of imported.signatures) if (!original.signatures.some((s) => Buffer.from(s.toXDR()).equals(Buffer.from(sig.toXDR())))) original.signatures.push(sig);
  return original;
}
