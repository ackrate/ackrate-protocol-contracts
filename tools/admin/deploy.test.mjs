import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { Account, Address, Keypair, Networks, SorobanDataBuilder, TransactionBuilder, xdr } from "@stellar/stellar-sdk";
import { deploymentOperation, eligibility, manifest, validate, V2, V2_ADMIN } from "./transfer.mjs";
import { predictedHelper, verifyBrowserBuild } from "./deploy.mjs";
import { verifyBuild, verifyRun, REPOSITORY } from "./github-build.mjs";
import { ADMIN_WASM_HASHES, ADMIN_WASM_FILES, HELPER_WASM_HASH, V2_WASM_HASH } from "./release.mjs";
const source = Keypair.random().publicKey(), governance = Keypair.random().publicKey();
const files = Object.fromEntries(await Promise.all(Object.keys(ADMIN_WASM_HASHES).map(async (name) => [name, await readFile(new URL(`../../contracts/admin/target/wasm32v1-none/release/${ADMIN_WASM_FILES[name]}`, import.meta.url))])));
const commit = "a".repeat(40);
const build = { version: 1, repository: REPOSITORY, commit, rust: "1.98.0", stellar: "28.1.0", hashes: ADMIN_WASM_HASHES, runId: "123", runUrl: `https://github.com/${REPOSITORY}/actions/runs/123` };
const m = manifest({ version: 1, network: "testnet", target: Address.contract(Buffer.alloc(32, 2)).toString(), helper: Address.contract(Buffer.alloc(32, 1)).toString(), initialAdmin: source, governance, helperWasmHash: HELPER_WASM_HASH, targetWasmHash: "b".repeat(64), delayLedgers: 30, maxFeeStroops: "500000000", deployment: { source, salt: "f".repeat(64) } });
const ctx = { m, passphrase: Networks.TESTNET, wasm: files.timelock };
function transaction(step, changed = ctx) {
  const op = deploymentOperation(changed, step);
  const host = op.body().invokeHostFunctionOp();
  if (step === "deploy-helper") host.auth([new xdr.SorobanAuthorizationEntry({ credentials: xdr.SorobanCredentials.sorobanCredentialsSourceAccount(), rootInvocation: new xdr.SorobanAuthorizedInvocation({ function: xdr.SorobanAuthorizedFunction.sorobanAuthorizedFunctionTypeCreateContractV2HostFn(host.hostFunction().createContractV2()), subInvocations: [] }) })]);
  return new TransactionBuilder(new Account(m.deployment.source, "10"), { fee: "10000", networkPassphrase: ctx.passphrase }).addOperation(op).setSorobanData(new SorobanDataBuilder().build()).setTimeout(300).build();
}
test("GitHub provenance rejects failed gates and a different source commit", () => {
  const run = { name: "Contract Gate Check", status: "completed", conclusion: "success", head_sha: commit, event: "pull_request", repository: { full_name: REPOSITORY }, head_repository: { full_name: REPOSITORY }, path: ".github/workflows/ci.yml" };
  const jobs = ["security", "fixed-target administration"].map((name) => ({ name, status: "completed", conclusion: "success" }));
  verifyRun(run, jobs, commit); verifyBuild(build, commit, files); verifyBrowserBuild(build, files.timelock);
  assert.throws(() => verifyRun({ ...run, conclusion: "failure" }, jobs, commit));
  assert.throws(() => verifyRun(run, jobs.slice(1), commit));
  assert.throws(() => verifyRun({ ...run, head_repository: { full_name: "fork/contracts" } }, jobs, commit));
  assert.throws(() => verifyBuild({ ...build, commit: "b".repeat(40) }, commit, files));
  assert.throws(() => verifyBuild(build, commit, { ...files, timelock: Buffer.from("wrong") }));
  assert.throws(() => verifyBrowserBuild(build, Buffer.from("wrong")));
});
test("deployment validates exact WASM, salt, target and governance", () => {
  validate(ctx, transaction("upload-helper"), "upload-helper");
  validate(ctx, transaction("deploy-helper"), "deploy-helper");
  assert.throws(() => validate(ctx, transaction("upload-helper", { ...ctx, wasm: Buffer.from("other") }), "upload-helper"), /preparation/);
  for (const change of [{ governance: source }, { target: m.helper }, { deployment: { ...m.deployment, salt: "e".repeat(64) } }]) assert.throws(() => validate(ctx, transaction("deploy-helper", { ...ctx, m: { ...m, ...change } }), "deploy-helper"), /arguments/);
  assert.notEqual(predictedHelper(m, Networks.PUBLIC, files.timelock), predictedHelper(m, Networks.TESTNET, files.timelock));
});
test("Mainnet recipient is an explicit separate trust input and files cannot override it", () => {
  const production = { ...m, network: "mainnet", target: V2, initialAdmin: V2_ADMIN, delayLedgers: 17280, targetWasmHash: V2_WASM_HASH };
  assert.throws(() => manifest(production), /recipient governance|expected governance/);
  manifest(production, governance);
  assert.throws(() => manifest({ ...production, governance: source }, governance), /recipient governance|expected governance/);
});
test("replacement blocks direct nomination and waits for old controller nomination", () => {
  const replacement = { ...m, initialAdmin: Address.contract(Buffer.alloc(32, 3)).toString() };
  const s = { owner: replacement.initialAdmin, pendingAdmin: null, config: { governance }, pendingGovernance: null, adopted: false, deployed: true };
  assert(!eligibility(replacement, s, "propose-admin").ready);
  assert(!eligibility(replacement, s, "accept-management").ready);
  s.pendingAdmin = m.helper; assert(eligibility(replacement, s, "accept-management").ready);
});
