import { Buffer } from "buffer";
import { Address, Keypair, hash, xdr } from "@stellar/stellar-sdk";
import { HELPER_WASM_HASH, V2_WASM_HASH } from "./release.mjs";
import { account, codeHash, context, deploymentOperation, manifest, read, state, V2 } from "./transfer.mjs";
export function verifyBrowserBuild(build, wasm) {
  if (build.version !== 1 || build.repository !== "ackrate/ackrate-protocol-contracts" || !/^[0-9a-f]{40}$/.test(build.commit) || !/^[1-9][0-9]*$/.test(build.runId) || build.runUrl !== `https://github.com/${build.repository}/actions/runs/${build.runId}` || build.hashes.timelock !== HELPER_WASM_HASH || Buffer.from(hash(wasm)).toString("hex") !== HELPER_WASM_HASH) throw new Error("Verified GitHub artifact and reviewed helper hash must agree");
}
export function predictedHelper(m, passphrase, wasm) {
  const preimage = deploymentOperation({ m, wasm }, "deploy-helper").body().invokeHostFunctionOp().hostFunction().createContractV2().contractIdPreimage();
  return Address.contract(hash(xdr.HashIdPreimage.envelopeTypeContractId(new xdr.HashIdPreimageContractId({ networkId: hash(Buffer.from(passphrase)), contractIdPreimage: preimage })).toXDR())).toString();
}
export async function planDeployment({ network, target, governance, source, delayLedgers, maxFeeStroops, salt }, build, wasm) {
  verifyBrowserBuild(build, wasm);
  Keypair.fromPublicKey(source); Keypair.fromPublicKey(governance); new Address(target);
  if (!target.startsWith("C")) throw new Error("Managed target must be a contract");
  const seed = { network, governance };
  const ctx = await context(seed);
  await Promise.all([account(ctx, source), account(ctx, governance)]);
  const [initialAdmin, pending, targetWasmHash] = await Promise.all([read(ctx, target, "get_admin"), read(ctx, target, "get_pending_admin"), codeHash(ctx, target)]);
  if (pending !== null) throw new Error("Managed target already has a pending administrator");
  if (network === "mainnet" && (target !== V2 || targetWasmHash !== V2_WASM_HASH)) throw new Error("Mainnet deployment must manage the reviewed V2 contract");
  let m = { version: 1, network, target, helper: Address.contract(Buffer.alloc(32)).toString(), initialAdmin, governance, delayLedgers, helperWasmHash: HELPER_WASM_HASH, targetWasmHash, maxFeeStroops, deployment: { source, salt }, build: { commit: build.commit, runId: build.runId } };
  if (initialAdmin.startsWith("C")) m.previousHelperWasmHash = await codeHash(ctx, initialAdmin);
  m.helper = predictedHelper(m, ctx.passphrase, wasm);
  m = manifest(m, governance); // Governance is a separate explicit user choice, never taken from an imported file.
  ctx.m = m; ctx.wasm = wasm;
  const current = await state(ctx);
  if (current.deployed) throw new Error("Predicted helper already exists; start a fresh deployment");
  return ctx;
}
