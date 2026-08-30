import { isConnected, requestAccess, signTransaction } from "@stellar/freighter-api";
import { Address, Networks, TransactionBuilder } from "@stellar/stellar-sdk";
import "./style.css";

const MAINNET = Networks.PUBLIC;
const byId = (id) => document.getElementById(id);
const xdrInput = byId("xdr");
const signButton = byId("sign");
let inspectedXdr = "";

function inspect(raw) {
  const tx = TransactionBuilder.fromXDR(raw.trim(), MAINNET);
  if (tx.operations.length !== 1 || tx.operations[0].type !== "invokeHostFunction") {
    throw new Error("Expected exactly one invokeHostFunction operation.");
  }
  const hostFunction = tx.operations[0].func;
  if (hostFunction.switch().name !== "hostFunctionTypeInvokeContract") {
    throw new Error("Expected an invoke-contract host function.");
  }
  const invocation = hostFunction.invokeContract();
  const rawFunctionName = invocation.functionName();
  const functionName = typeof rawFunctionName === "string"
    ? rawFunctionName
    : new TextDecoder().decode(rawFunctionName);
  const args = invocation.args();
  if (functionName !== "upgrade" || args.length !== 1 || args[0].switch().name !== "scvBytes") {
    throw new Error("Expected only upgrade(new_wasm_hash: BytesN<32>).");
  }
  const wasmHash = args[0].bytes();
  if (wasmHash.length !== 32) throw new Error("Upgrade WASM hash is not 32 bytes.");
  const hash = [...tx.hash()].map((byte) => byte.toString(16).padStart(2, "0")).join("");
  byId("network").textContent = "Public Global Stellar Network";
  byId("source").textContent = tx.source;
  byId("sequence").textContent = tx.sequence;
  byId("fee").textContent = `${tx.fee} stroops`;
  byId("signatures").textContent = String(tx.signatures.length);
  byId("hash").textContent = hash;
  byId("contract").textContent = Address.fromScAddress(invocation.contractAddress()).toString();
  byId("function").textContent = functionName;
  byId("wasm-hash").textContent = [...wasmHash]
    .map((byte) => byte.toString(16).padStart(2, "0")).join("");
  byId("review").classList.remove("hidden");
  inspectedXdr = raw.trim();
  signButton.disabled = false;
}

byId("inspect").addEventListener("click", () => {
  try {
    inspect(xdrInput.value);
    byId("status").textContent = "Decoded as a mainnet transaction. Review every field before signing.";
  } catch (error) {
    inspectedXdr = "";
    signButton.disabled = true;
    byId("review").classList.add("hidden");
    byId("status").textContent = `Invalid mainnet transaction XDR: ${error.message}`;
  }
});

signButton.addEventListener("click", async () => {
  try {
    if (xdrInput.value.trim() !== inspectedXdr) throw new Error("XDR changed after inspection; inspect it again.");
    const connected = await isConnected();
    if (!connected.isConnected) throw new Error("Freighter is not installed or available.");
    const access = await requestAccess();
    if (access.error) throw new Error(access.error);
    const signed = await signTransaction(inspectedXdr, {
      networkPassphrase: MAINNET,
      address: access.address,
    });
    if (signed.error) throw new Error(signed.error);
    byId("signed-xdr").value = signed.signedTxXdr;
    byId("result").classList.remove("hidden");
    byId("status").textContent = `Signed by ${access.address}. Send only the signed envelope to the coordinator.`;
  } catch (error) {
    byId("status").textContent = error.message;
  }
});

byId("copy").addEventListener("click", async () => {
  await navigator.clipboard.writeText(byId("signed-xdr").value);
  byId("status").textContent = "Signed XDR copied.";
});
