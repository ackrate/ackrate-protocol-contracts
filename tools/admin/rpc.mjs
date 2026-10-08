import { ADMIN_WASM_FILES } from "./release.mjs";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { Address, Contract, Networks, Operation, TransactionBuilder, nativeToScVal, rpc, scValToNative, xdr } from "@stellar/stellar-sdk";

export { Address, Contract, Networks, Operation, TransactionBuilder, nativeToScVal, rpc, scValToNative, xdr };
export const bytes = (b) => nativeToScVal(b, { type: "bytes" });
export const u32 = (n) => nativeToScVal(n, { type: "u32" });
export const address = (id) => new Address(id).toScVal();
export const actionValue = (action) => {
  const [kind, ...args] = action;
  const symbol = nativeToScVal(kind, { type: "symbol" });
  if (kind === "Upgrade" && args.length === 1 && /^[0-9a-f]{64}$/i.test(args[0])) return xdr.ScVal.scvVec([symbol, bytes(Buffer.from(args[0], "hex"))]);
  if (kind === "Unpause" && args.length === 0) return xdr.ScVal.scvVec([symbol]);
  if (kind === "AssetPolicy" && args.length === 2 && typeof args[1] === "boolean") return xdr.ScVal.scvVec([symbol, address(args[0]), nativeToScVal(args[1])]);
  if (kind === "TransferAdmin" && args.length === 1) return xdr.ScVal.scvVec([symbol, address(args[0])]);
  throw new Error("Invalid typed admin action");
};
export async function connect(network) {
  if (!["testnet", "mainnet"].includes(network)) throw new Error("Explicit testnet or mainnet required");
  const passphrase = network === "testnet" ? Networks.TESTNET : Networks.PUBLIC;
  const server = new rpc.Server(network === "testnet" ? "https://soroban-testnet.stellar.org" : "https://mainnet.sorobanrpc.com");
  if ((await server.getNetwork()).passphrase !== passphrase) throw new Error("RPC network mismatch");
  return { network, passphrase, server };
}
export async function build(ctx, source, operation) {
  return new TransactionBuilder(await ctx.server.getAccount(source), { fee: ctx.inclusionFee ?? "100", networkPassphrase: ctx.passphrase })
    .addOperation(operation).setTimeout(1800).build();
}
export async function read(ctx, source, contract, method, ...args) {
  const simulation = await ctx.server.simulateTransaction(await build(ctx, source, new Contract(contract).call(method, ...args)));
  if (!rpc.Api.isSimulationSuccess(simulation) || !simulation.result?.retval) throw new Error(`Read ${method} failed: ${simulation.error ?? "no result"}`);
  return simulation.result.retval;
}
export async function wait(ctx, hash) {
  for (let i = 0; i < 60; i++) {
    const result = await ctx.server.getTransaction(hash);
    if (["SUCCESS", "FAILED"].includes(result.status)) return result;
    await new Promise((resolve) => setTimeout(resolve, 1500));
  }
  throw new Error(`Uncertain submission ${hash}; query it before any retry`);
}
export async function wasm(name) {
  const value = await readFile(new URL(`../../contracts/admin/target/wasm32v1-none/release/${ADMIN_WASM_FILES[name]}`, import.meta.url));
  return { value, hash: createHash("sha256").update(value).digest() };
}
