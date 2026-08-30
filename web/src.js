import {
  getNetworkDetails,
  isConnected,
  requestAccess,
  signTransaction,
} from "@stellar/freighter-api";
import {
  Address,
  Contract,
  Networks,
  Operation,
  StrKey,
  TransactionBuilder,
  nativeToScVal,
  rpc,
} from "@stellar/stellar-sdk";
import "./style.css";

const NETWORKS = {
  mainnet: {
    label: "Production / Mainnet",
    passphrase: Networks.PUBLIC,
    horizon: "https://horizon.stellar.org",
    rpc: "https://mainnet.sorobanrpc.com",
  },
  testnet: {
    label: "Testnet",
    passphrase: Networks.TESTNET,
    horizon: "https://horizon-testnet.stellar.org",
    rpc: "https://soroban-testnet.stellar.org",
  },
};
const WASM_URL = "https://raw.githubusercontent.com/ackrate/ackrate-protocol-contracts/v3mainnet/web/public/mandate_registry.wasm";
const EXPECTED_WASM_HASH = "b9e4e607ab56e63ce7d5e75ff192e56ccb3cf741cb78c0944c7004ac3f9487ca";
const byId = (id) => document.getElementById(id);
const xdrInput = byId("xdr");
const signButton = byId("sign");
let inspectedXdr = "";

const network = () => NETWORKS[byId("environment").value];
const rpcServer = () => new rpc.Server(network().rpc, { allowHttp: false });
const toHex = (bytes) => [...bytes]
  .map((byte) => byte.toString(16).padStart(2, "0")).join("");

function fromHex(value) {
  const normalized = value.trim().toLowerCase();
  if (!/^[0-9a-f]{64}$/.test(normalized)) throw new Error("WASM hash must be exactly 64 hexadecimal characters.");
  return Uint8Array.from(normalized.match(/.{2}/g), (byte) => Number.parseInt(byte, 16));
}

function setNetworkUi() {
  const selected = network();
  byId("horizon-url").textContent = selected.horizon;
  byId("rpc-url").textContent = selected.rpc;
  byId("fund-testnet").classList.toggle("hidden", byId("environment").value !== "testnet");
}

function makeShareUrl(xdr) {
  const fragment = new URLSearchParams({ network: byId("environment").value, xdr });
  return `${location.origin}${location.pathname}#${fragment}`;
}

function loadShareUrl() {
  const params = new URLSearchParams(location.hash.slice(1));
  const encodedXdr = params.get("xdr");
  if (!encodedXdr) return false;
  const selectedNetwork = params.get("network");
  if (!NETWORKS[selectedNetwork]) throw new Error("Unknown network in cosigner URL.");
  byId("environment").value = selectedNetwork;
  setNetworkUi();
  xdrInput.value = encodedXdr;
  inspect(encodedXdr);
  byId("status").textContent = "Cosigner transaction loaded from this URL. Verify every field before signing.";
  return true;
}

function inspect(raw) {
  const selected = network();
  const tx = TransactionBuilder.fromXDR(raw.trim(), selected.passphrase);
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
  byId("network").textContent = selected.label;
  byId("review-rpc").textContent = selected.rpc;
  byId("source").textContent = tx.source;
  byId("sequence").textContent = tx.sequence;
  byId("fee").textContent = `${tx.fee} stroops`;
  byId("signatures").textContent = String(tx.signatures.length);
  byId("tx-hash").textContent = toHex(tx.hash());
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

async function assertNetwork() {
  const selected = network();
  const [rpcNetwork, horizonResponse] = await Promise.all([
    rpcServer().getNetwork(),
    fetch(selected.horizon),
  ]);
  if (rpcNetwork.passphrase !== selected.passphrase) throw new Error("RPC network passphrase mismatch.");
  if (!horizonResponse.ok) throw new Error(`Horizon returned HTTP ${horizonResponse.status}.`);
  return selected;
}

async function connectedWallet() {
  const connected = await isConnected();
  if (!connected.isConnected) throw new Error("Freighter is not installed or available.");
  const access = await requestAccess();
  if (access.error) throw new Error(access.error);
  const details = await getNetworkDetails();
  if (details.error) throw new Error(details.error);
  if (details.networkPassphrase !== network().passphrase) {
    throw new Error(`Switch Freighter to ${network().label} before continuing.`);
  }
  return access.address;
}

async function waitForTransaction(server, hash) {
  for (let attempt = 0; attempt < 45; attempt += 1) {
    await new Promise((resolve) => setTimeout(resolve, 2000));
    const result = await server.getTransaction(hash);
    if (result.status === "SUCCESS") return result;
    if (result.status === "FAILED") throw new Error(`Transaction failed: ${hash}`);
  }
  throw new Error(`Transaction is still pending: ${hash}`);
}

async function freighterTransaction(operation, wallet, message) {
  const selected = network();
  const server = rpcServer();
  const account = await server.getAccount(wallet);
  const transaction = new TransactionBuilder(account, {
    fee: "100",
    networkPassphrase: selected.passphrase,
  }).addOperation(operation).setTimeout(900).build();
  const prepared = await server.prepareTransaction(transaction);
  byId("status").textContent = message;
  const signed = await signTransaction(prepared.toXDR(), {
    networkPassphrase: selected.passphrase,
    address: wallet,
  });
  if (signed.error) throw new Error(signed.error);
  const envelope = TransactionBuilder.fromXDR(signed.signedTxXdr, selected.passphrase);
  const sent = await server.sendTransaction(envelope);
  if (sent.status === "ERROR") throw new Error(sent.errorResult?.toString() || "RPC rejected the transaction.");
  return waitForTransaction(server, sent.hash || toHex(envelope.hash()));
}

async function reviewedWasm() {
  const response = await fetch(WASM_URL, { cache: "no-store" });
  if (!response.ok) throw new Error(`GitHub WASM download returned HTTP ${response.status}.`);
  const wasm = new Uint8Array(await response.arrayBuffer());
  const hash = new Uint8Array(await crypto.subtle.digest("SHA-256", wasm));
  if (toHex(hash) !== EXPECTED_WASM_HASH) throw new Error("GitHub WASM does not match the pinned reviewed SHA-256.");
  return { wasm, hash };
}

byId("environment").addEventListener("change", () => {
  setNetworkUi();
  inspectedXdr = "";
  signButton.disabled = true;
  byId("status").textContent = `Selected ${network().label}. Check Freighter is on the same network.`;
});

byId("check-network").addEventListener("click", async () => {
  try {
    await assertNetwork();
    byId("status").textContent = `${network().label} Horizon and Stellar RPC are reachable and agree on the expected network.`;
  } catch (error) {
    byId("status").textContent = `Network check failed: ${error.message}`;
  }
});

byId("fund-testnet").addEventListener("click", async () => {
  try {
    if (byId("environment").value !== "testnet") throw new Error("Faucet funding is testnet-only.");
    await assertNetwork();
    const wallet = await connectedWallet();
    byId("status").textContent = `Requesting testnet faucet funds for ${wallet}…`;
    await rpcServer().fundAddress(wallet);
    byId("status").textContent = `Testnet wallet funded: ${wallet}`;
  } catch (error) {
    byId("status").textContent = `Testnet funding failed: ${error.message}`;
  }
});

byId("deploy").addEventListener("click", async () => {
  try {
    const admin = byId("deploy-admin").value.trim();
    if (!StrKey.isValidEd25519PublicKey(admin)) throw new Error("Future admin must be a valid G-account.");
    const selected = await assertNetwork();
    const wallet = await connectedWallet();
    const accountResponse = await fetch(`${selected.horizon}/accounts/${wallet}`);
    if (!accountResponse.ok) throw new Error(`Freighter account is not funded on ${selected.label}.`);
    const { wasm, hash } = await reviewedWasm();

    await freighterTransaction(
      Operation.uploadContractWasm({ wasm }),
      wallet,
      "Approve the reviewed WASM upload in Freighter…",
    );

    const deployment = await freighterTransaction(
      Operation.createCustomContract({
        address: new Address(wallet),
        wasmHash: hash,
        salt: crypto.getRandomValues(new Uint8Array(32)),
        constructorArgs: [new Address(admin).toScVal()],
      }),
      wallet,
      "Approve initial deployment with the selected admin in Freighter…",
    );
    if (!deployment.returnValue) throw new Error("Deployment succeeded without a returned contract address.");
    const contractId = Address.fromScVal(deployment.returnValue).toString();
    byId("deployed-contract").textContent = contractId;
    byId("deployed-hash").textContent = toHex(hash);
    byId("deployment-tx").textContent = deployment.txHash;
    byId("deployment-result").classList.remove("hidden");
    byId("contract-input").value = contractId;
    byId("admin").value = admin;
    byId("hash-input").value = toHex(hash);
    byId("status").textContent = `Initial SimpleContract deployed on ${selected.label}: ${contractId}`;
  } catch (error) {
    byId("status").textContent = `Deployment failed: ${error.message}`;
  }
});

byId("prepare").addEventListener("click", async () => {
  try {
    const admin = byId("admin").value.trim();
    const contractId = byId("contract-input").value.trim();
    const wasmHash = fromHex(byId("hash-input").value);
    if (!StrKey.isValidEd25519PublicKey(admin)) throw new Error("Admin must be a valid G-account.");
    if (!StrKey.isValidContract(contractId)) throw new Error("Contract must be a valid C-address.");
    await assertNetwork();
    byId("status").textContent = "Reading the admin account and simulating through RPC…";
    const server = rpcServer();
    const account = await server.getAccount(admin);
    const operation = new Contract(contractId).call("upgrade", nativeToScVal(wasmHash, { type: "bytes" }));
    const transaction = new TransactionBuilder(account, {
      fee: "100",
      networkPassphrase: network().passphrase,
    }).addOperation(operation).setTimeout(900).build();
    const prepared = await server.prepareTransaction(transaction);
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
    byId("status").textContent = `Decoded as one ${network().label} SimpleContract upgrade. Verify every field before signing.`;
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
    const wallet = await connectedWallet();
    const signed = await signTransaction(inspectedXdr, {
      networkPassphrase: network().passphrase,
      address: wallet,
    });
    if (signed.error) throw new Error(signed.error);
    xdrInput.value = signed.signedTxXdr;
    inspect(signed.signedTxXdr);
    const shareUrl = makeShareUrl(signed.signedTxXdr);
    byId("share-url").value = shareUrl;
    history.replaceState(null, "", shareUrl);
    byId("status").textContent = `Signature appended by ${wallet}. Copy this new URL for the next cosigner.`;
  } catch (error) {
    byId("status").textContent = `Signing failed: ${error.message}`;
  }
});

byId("copy-url").addEventListener("click", async () => {
  await navigator.clipboard.writeText(byId("share-url").value);
  byId("status").textContent = "Cosigner URL copied. It contains the network, transaction, and signatures in its fragment.";
});

byId("submit").addEventListener("click", async () => {
  try {
    const transaction = inspect(xdrInput.value);
    if (transaction.signatures.length < 2) throw new Error("Two signatures are required before submission.");
    await assertNetwork();
    byId("status").textContent = "Submitting the signed envelope through RPC…";
    const sent = await rpcServer().sendTransaction(transaction);
    if (sent.status === "ERROR") throw new Error(sent.errorResult?.toString() || "RPC rejected the transaction.");
    const txHash = sent.hash || toHex(transaction.hash());
    const result = await waitForTransaction(rpcServer(), txHash);
    byId("status").textContent = `Upgrade confirmed on ${network().label}. Transaction: ${result.txHash}`;
  } catch (error) {
    byId("status").textContent = `Submission failed: ${error.message}`;
  }
});

setNetworkUi();
try {
  loadShareUrl();
} catch (error) {
  byId("status").textContent = `Could not load cosigner URL: ${error.message}`;
}
