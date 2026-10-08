import test from "node:test";
import assert from "node:assert/strict";
import { Account, Address, Contract, Keypair, Networks, SorobanDataBuilder, TransactionBuilder, xdr } from "@stellar/stellar-sdk";
import { eligibility, identifyStep, manifest, mergeSignatures, signatures, specification, validate, V2, V2_ADMIN } from "./transfer.mjs";
import { HELPER_WASM_HASH, V2_WASM_HASH } from "./release.mjs";

const a = Keypair.random(), b = Keypair.random(), c = Keypair.random();
const helper = Address.contract(Buffer.alloc(32, 1)).toString(), target = Address.contract(Buffer.alloc(32, 2)).toString();
const m = manifest({ version: 1, network: "testnet", target, helper, initialAdmin: a.publicKey(), governance: b.publicKey(), helperWasmHash: "1".repeat(64), targetWasmHash: "2".repeat(64), delayLedgers: 30, maxFeeStroops: "10000000" });
const ctx = { m, passphrase: Networks.TESTNET };
test("Mainnet rejects unreviewed code and remains disabled without a pinned recipient", () => {
  const production = { ...m, network: "mainnet", target: V2, initialAdmin: V2_ADMIN, delayLedgers: 17280, helperWasmHash: HELPER_WASM_HASH, targetWasmHash: V2_WASM_HASH };
  assert.throws(() => manifest({ ...production, helperWasmHash: "0".repeat(64) }), /WASM pins/);
  assert.throws(() => manifest({ ...production, targetWasmHash: "0".repeat(64) }), /WASM pins/);
  assert.throws(() => manifest(production), /recipient governance|expected governance/);
});
const policy = { signers: [a, b, c].map((k) => ({ key: k.publicKey(), weight: 1 })), thresholds: { med_threshold: 2 } };
function transaction(step = "propose-admin", alter = {}) {
  const spec = { ...specification(m, step), ...alter };
  const op = new Contract(spec.contract).call(spec.method, ...spec.args);
  const host = op.body().invokeHostFunctionOp();
  host.auth([new xdr.SorobanAuthorizationEntry({ credentials: xdr.SorobanCredentials.sorobanCredentialsSourceAccount(), rootInvocation: new xdr.SorobanAuthorizedInvocation({ function: xdr.SorobanAuthorizedFunction.sorobanAuthorizedFunctionTypeContractFn(host.hostFunction().invokeContract()), subInvocations: [] }) })]);
  return new TransactionBuilder(new Account(spec.source, "10"), { fee: "10000", networkPassphrase: Networks.TESTNET }).addOperation(op).setSorobanData(new SorobanDataBuilder().build()).setTimeout(300).build();
}
test("two different valid signatures meet quorum; duplicate signatures never add weight", () => {
  const tx = transaction(); tx.sign(a); assert.equal(signatures(tx, policy).weight, 1);
  tx.sign(a); assert.equal(signatures(tx, policy).weight, 1);
  tx.sign(c); assert.equal(signatures(tx, policy).weight, 2);
});
test("unrelated and forged signatures are rejected", () => {
  const tx = transaction(); tx.sign(Keypair.random()); assert.throws(() => signatures(tx, policy), /unrelated/);
});
test("default account thresholds still require a verified signature", () => {
  const tx = transaction();
  const single = { signers: [{ key: a.publicKey(), weight: 1 }], thresholds: { med_threshold: 0 } };
  assert.deepEqual(signatures(tx, single), { weight: 0, required: 1, signers: [] });
  tx.sign(a); assert.equal(signatures(tx, single).weight, 1);
});
test("transfer validator rejects changed destinations, sources and bodies", () => {
  const tx = transaction(); validate(ctx, tx, "propose-admin");
  assert.throws(() => validate(ctx, transaction("propose-admin", { args: [new Address(target).toScVal()] }), "propose-admin"), /arguments/);
  assert.throws(() => validate(ctx, transaction("propose-admin", { source: c.publicKey() }), "propose-admin"), /source/);
  assert.throws(() => validate(ctx, tx, "propose-admin", "0".repeat(64)), /body changed/);
});
test("merging preserves earlier signatures and rejects another transaction", () => {
  const tx = transaction(); tx.sign(a);
  const imported = TransactionBuilder.fromXDR(tx.toXDR(), Networks.TESTNET); imported.sign(b);
  mergeSignatures(tx, imported, ctx, "propose-admin"); assert.equal(signatures(tx, policy).weight, 2);
  assert.throws(() => mergeSignatures(tx, transaction("accept-management"), ctx, "propose-admin"));
});
test("authority handoff is completed before nomination; adoption is final", () => {
  const s = { config: { governance: a.publicKey() }, owner: m.initialAdmin, pendingAdmin: null, pendingGovernance: m.governance, adopted: false };
  assert(eligibility(m, s, "accept-governance").ready); assert(!eligibility(m, s, "propose-admin").ready);
  s.config.governance = m.governance; s.pendingGovernance = null;
  assert(eligibility(m, s, "propose-admin").ready);
  s.pendingAdmin = m.helper; assert(eligibility(m, s, "accept-management").ready);
  s.owner = m.helper; s.adopted = true; assert(eligibility(m, s, "accept-management").done);
});

test("co-signer errors retain expiry or fee diagnosis after identifying an allowed call", () => {
  const tx = transaction();
  assert.equal(identifyStep(ctx, tx, ["propose-admin", "accept-management"]), "propose-admin");
  assert.throws(() => validate({ ...ctx, m: { ...m, maxFeeStroops: "1" } }, tx, "propose-admin"), /Quoted maximum fee/);
});
