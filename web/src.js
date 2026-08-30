import { isConnected, requestAccess, signTransaction } from "@stellar/freighter-api";
import {
  Address,
  Contract,
  Networks,
  StrKey,
  TransactionBuilder,
  nativeToScVal,
  rpc,
} from "@stellar/stellar-sdk";
import "./style.css";

const MAINNET = Networks.PUBLIC;
const DEFAULT_RPC = "https://mainnet.sorobanrpc.com";
const byId = (id) => document.getElementById(id);
const xdrInput = byId("xdr");
const signButton = byId("sign");
let inspectedXdr = "";

const toHex = (bytes) => [...bytes]
  .map((byte) => byte.toString(16).padStart(2, "0")).join("");

function fromHex(value) {
  const normalized = value.trim().toLowerCase();
  if (!/^[0-9a-f]{64}$/.test(normalized)) throw new Error("WASM hash must be exactly 64 hexadecimal characters.");
  return Uint8Array.from(normalized.match(/.{2}/g), (byte) => Number.parseInt(byte, 16));
}

function rpcUrl() {
  const url = new URL(byId("rpc").value.trim() || DEFAULT_RPC);
  if (url.protocol !== "https:") throw new Error("Mainnet RPC must use HTTPS.");
  return url.toString().replace(/\/$/, "");
}

function server() {
  return new rpc.Server(rpcUrl(), { allowHttp: false });
}

function makeShareUrl(xdr) {
  const fragment = new URLSearchParams({ rpc: rpcUrl(), xdr });
  return `${location.origin}${location.pathname}#${fragment}`;
}

function loadShareUrl() {
  const params = new URLSearchParams(location.hash.slice(1));
  const encodedRpc = params.get("rpc");
  const encodedXdr = params.get("xdr");
  if (!encodedXdr) return false;
  byId("rpc").value = encodedRpc || DEFAULT_RPC;
  xdrInput.value = encodedXdr;
  inspect(encodedXdr);
  byId("status").textContent = "Cosigner transaction loaded from this URL. Verify every field before signing.";
  return true;
}

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

  const contractId = Address.fromScAddress(invocation.contractAddress()).toString();
  const txHash = toHex(tx.hash());
  byId("network").textContent = "Public Global Stellar Network";
  byId("review-rpc").textContent = rpcUrl();
  byId("source").textContent = tx.source;
  byId("sequence").textContent = tx.sequence;
  byId("fee").textContent = `${tx.fee} stroops`;
  byId("signatures").textContent = String(tx.signatures.length);
  byId("tx-hash").textContent = txHash;
  byId("contract").textContent = contractId;
  byId("function").textContent = functionName;
  byId("wasm-hash").textContent = toHex(wasmHash);
  byId("admin").value ||= tx.source;
  byId("contract-input").value ||= contractId;
  byId("hash-input").value ||= toHex(wasmHash);
  byId("review").classList.remove("hidden");
  byId("share").classList.remove("hidden");
  byId("share-url").value = makeShareUrl(raw.trim());
  inspectedXdr = raw.trim();
  signButton.disabled = false;
  return tx;
}

async function assertMainnetRpc(rpcServer) {
  const network = await rpcServer.getNetwork();
  if (network.passphrase !== MAINNET) throw new Error("RPC is not serving Stellar mainnet.");
}

byId("prepare").addEventListener("click", async () => {
  try {
    const admin = byId("admin").value.trim();
    const contractId = byId("contract-input").value.trim();
    const wasmHash = fromHex(byId("hash-input").value);
    if (!StrKey.isValidEd25519PublicKey(admin)) throw new Error("Admin must be a valid G-account.");
    if (!StrKey.isValidContract(contractId)) throw new Error("Contract must be a valid C-address.");

    byId("status").textContent = "Reading the admin account and simulating through RPC…";
    const rpcServer = server();
    await assertMainnetRpc(rpcServer);
    const account = await rpcServer.getAccount(admin);
    const operation = new Contract(contractId).call(
      "upgrade",
      nativeToScVal(wasmHash, { type: "bytes" }),
    );
    const transaction = new TransactionBuilder(account, {
      fee: "100",
      networkPassphrase: MAINNET,
    }).addOperation(operation).setTimeout(900).build();
    const prepared = await rpcServer.prepareTransaction(transaction);
    const xdr = prepared.toXDR();
    xdrInput.value = xdr;
    inspect(xdr);
    history.replaceState(null, "", makeShareUrl(xdr));
    byId("status").textContent = "RPC simulation succeeded. Review, sign, then copy the generated cosigner URL.";
  } catch (error) {
    byId("status").textContent = `Preparation failed: ${error.message}`;
  }
});

byId("inspect").addEventListener("click", () => {
  try {
    inspect(xdrInput.value);
    byId("status").textContent = "Decoded as one mainnet SimpleContract upgrade. Verify every field before signing.";
  } catch (error) {
    inspectedXdr = "";
    signButton.disabled = true;
    byId("review").classList.add("hidden");
    byId("share").classList.add("hidden");
    byId("status").textContent = `Invalid upgrade transaction: ${error.message}`;
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
    xdrInput.value = signed.signedTxXdr;
    inspect(signed.signedTxXdr);
    const shareUrl = makeShareUrl(signed.signedTxXdr);
    byId("share-url").value = shareUrl;
    history.replaceState(null, "", shareUrl);
    byId("status").textContent = `Signature appended by ${access.address}. Copy this new URL for the next cosigner.`;
  } catch (error) {
    byId("status").textContent = `Signing failed: ${error.message}`;
  }
});

byId("copy-url").addEventListener("click", async () => {
  await navigator.clipboard.writeText(byId("share-url").value);
  byId("status").textContent = "Cosigner URL copied. It contains the transaction and attached signatures in its fragment.";
});

byId("submit").addEventListener("click", async () => {
  try {
    const transaction = inspect(xdrInput.value);
    if (transaction.signatures.length < 2) throw new Error("Two signatures are required before submission.");
    const rpcServer = server();
    await assertMainnetRpc(rpcServer);
    byId("status").textContent = "Submitting the signed envelope through RPC…";
    const sent = await rpcServer.sendTransaction(transaction);
    if (sent.status === "ERROR") throw new Error(sent.errorResult?.toString() || "RPC rejected the transaction.");
    const txHash = sent.hash || toHex(transaction.hash());
    for (let attempt = 0; attempt < 30; attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 2000));
      const result = await rpcServer.getTransaction(txHash);
      if (result.status === "SUCCESS") {
        byId("status").textContent = `Upgrade confirmed through RPC. Transaction: ${txHash}`;
        return;
      }
      if (result.status === "FAILED") throw new Error(`Transaction failed: ${txHash}`);
    }
    byId("status").textContent = `Submitted and still pending. Query this hash through RPC: ${txHash}`;
  } catch (error) {
    byId("status").textContent = `Submission failed: ${error.message}`;
  }
});

try {
  loadShareUrl();
} catch (error) {
  byId("status").textContent = `Could not load cosigner URL: ${error.message}`;
}
