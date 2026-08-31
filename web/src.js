import {
  getNetworkDetails,
  isConnected,
  requestAccess,
  signTransaction,
} from "@stellar/freighter-api";
import {
  Address,
  Contract,
  Keypair,
  Networks,
  Operation,
  StrKey,
  TransactionBuilder,
  nativeToScVal,
  rpc,
  scValToNative,
  xdr,
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
const EXPECTED_WASM_HASH = "5b0173d49c836ef756c96bee143b46b4bf956be19dee3a1d50498c0cc4c32cad";
const TRUSTED_BUILDER = "https://github.com/stellar-expert/soroban-build-workflow/.github/workflows/release.yml@88068ec50cba931a96436869727ed08edeb76ade";
const byId = (id) => document.getElementById(id);
const xdrInput = byId("xdr");
const signButton = byId("sign");
let inspectedXdr = "";
const LOCAL_IDS_KEY = "ackrate-simple-contract-ids-v1";
const REMEMBERED_IDS = [
  "environment",
  "deploy-admin",
  "git-ref",
  "policy-account",
  "signer-2",
  "signer-3",
  "admin",
  "contract-input",
  "hash-input",
  "new-admin",
];

const network = () => NETWORKS[byId("environment").value];
const rpcServer = () => new rpc.Server(network().rpc, { allowHttp: false });
const explorerTransactionUrl = (hash) => `https://stellar.expert/explorer/${byId("environment").value === "mainnet" ? "public" : "testnet"}/tx/${hash}`;
const toHex = (bytes) => [...bytes]
  .map((byte) => byte.toString(16).padStart(2, "0")).join("");

function fromHex(value) {
  const normalized = value.trim().toLowerCase();
  if (!/^[0-9a-f]{64}$/.test(normalized)) throw new Error("WASM hash must be exactly 64 hexadecimal characters.");
  return Uint8Array.from(normalized.match(/.{2}/g), (byte) => Number.parseInt(byte, 16));
}

function setStatus(message, transactionHash = "") {
  const status = byId("status");
  status.replaceChildren(document.createTextNode(message));
  status.setAttribute("aria-busy", transactionHash ? "false" : /…$/.test(message) ? "true" : "false");
  if (transactionHash) {
    status.append(document.createTextNode(" "));
    const link = document.createElement("a");
    link.href = explorerTransactionUrl(transactionHash);
    link.target = "_blank";
    link.rel = "noreferrer";
    link.textContent = `View transaction ${transactionHash.slice(0, 10)}…`;
    status.append(link);
  }
}

function setTransactionLink(id, hash) {
  const link = byId(id);
  link.href = explorerTransactionUrl(hash);
  link.textContent = hash;
}

function persistPublicIds() {
  try {
    const values = Object.fromEntries(REMEMBERED_IDS.map((id) => [id, byId(id).value.trim()]));
    localStorage.setItem(LOCAL_IDS_KEY, JSON.stringify(values));
  } catch {
    // Storage can be disabled; the app remains fully usable without persistence.
  }
}

function restorePublicIds() {
  try {
    const values = JSON.parse(localStorage.getItem(LOCAL_IDS_KEY) || "{}");
    REMEMBERED_IDS.forEach((id) => {
      if (typeof values[id] === "string" && values[id]) byId(id).value = values[id];
    });
  } catch {
    localStorage.removeItem(LOCAL_IDS_KEY);
  }
}

REMEMBERED_IDS.forEach((id) => {
  byId(id).addEventListener(id === "environment" ? "change" : "input", persistPublicIds);
});

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
  const transaction = inspect(encodedXdr);
  readAccountPolicy(transaction.source).then((policy) => {
    byId("policy-account").value = transaction.source;
    byId("policy-result").textContent = JSON.stringify(policy, null, 2);
  }).catch(() => {});
  setStatus("Cosigner transaction loaded from this URL. Verify every field before signing.");
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
  let operationArgument = "none";
  if (functionName === "upgrade") {
    if (args.length !== 1 || args[0].switch().name !== "scvBytes" || args[0].bytes().length !== 32) {
      throw new Error("Expected upgrade(new_wasm_hash: BytesN<32>).");
    }
    operationArgument = toHex(args[0].bytes());
  } else if (functionName === "set_admin") {
    if (args.length !== 1 || args[0].switch().name !== "scvAddress") {
      throw new Error("Expected set_admin(new_admin: Address).");
    }
    operationArgument = Address.fromScVal(args[0]).toString();
  } else if ((functionName === "pause" || functionName === "unpause") && args.length === 0) {
    operationArgument = "none";
  } else {
    throw new Error("Only upgrade, set_admin, pause, and unpause admin calls are allowed.");
  }

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
  byId("operation-argument").textContent = operationArgument;
  byId("admin").value ||= tx.source;
  byId("contract-input").value ||= contractId;
  if (functionName === "upgrade") byId("hash-input").value ||= operationArgument;
  if (functionName === "set_admin") byId("new-admin").value ||= operationArgument;
  persistPublicIds();
  byId("review").classList.remove("hidden");
  byId("share").classList.remove("hidden");
  byId("share-url").value = makeShareUrl(raw.trim());
  inspectedXdr = raw.trim();
  signButton.disabled = false;
  return tx;
}

async function assertNetwork() {
  setStatus(`Checking ${network().label} Horizon and Stellar RPC…`);
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
  setStatus(`Waiting for Freighter on ${network().label}…`);
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

async function readAccountPolicy(account) {
  if (!StrKey.isValidEd25519PublicKey(account)) throw new Error("Policy account must be a valid G-account.");
  const ledgerKey = xdr.LedgerKey.account(new xdr.LedgerKeyAccount({
    accountId: Keypair.fromPublicKey(account).xdrAccountId(),
  }));
  const response = await rpcServer().getLedgerEntries(ledgerKey);
  if (response.entries.length !== 1) throw new Error("Account was not found through RPC.");
  const entry = response.entries[0].val.account();
  const thresholds = [...entry.thresholds()];
  const signers = entry.signers().map((signer) => {
    const key = signer.key();
    return {
      key: key.switch().name === "signerKeyTypeEd25519"
        ? StrKey.encodeEd25519PublicKey(key.ed25519())
        : key.switch().name,
      weight: signer.weight(),
    };
  });
  return {
    account,
    masterWeight: thresholds[0],
    lowThreshold: thresholds[1],
    mediumThreshold: thresholds[2],
    highThreshold: thresholds[3],
    signers,
  };
}

function assertTwoOfThree(policy) {
  const ed25519 = policy.signers.filter((signer) => signer.key.startsWith("G") && signer.weight === 1);
  if (
    policy.masterWeight !== 1
    || policy.lowThreshold !== 2
    || policy.mediumThreshold !== 2
    || policy.highThreshold !== 2
    || ed25519.length !== 2
    || policy.signers.length !== 2
  ) {
    throw new Error("Account is not exactly three weight-1 keys with 2/2/2 thresholds.");
  }
  return policy;
}

async function waitForTransaction(server, hash) {
  setStatus(`Transaction ${hash.slice(0, 10)}… submitted; waiting for RPC confirmation…`);
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
  setStatus(message);
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

async function freighterClassicTransaction(operations, wallet, message) {
  const selected = network();
  const server = rpcServer();
  const account = await server.getAccount(wallet);
  const builder = new TransactionBuilder(account, {
    fee: String(100 * operations.length),
    networkPassphrase: selected.passphrase,
  }).setTimeout(900);
  operations.forEach((operation) => builder.addOperation(operation));
  const transaction = builder.build();
  setStatus(message);
  const signed = await signTransaction(transaction.toXDR(), {
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
  const ref = byId("git-ref").value.trim();
  if (!ref || ref.includes("..") || !/^[A-Za-z0-9._/-]+$/.test(ref)) {
    throw new Error("Git ref contains unsupported characters.");
  }
  const commitResponse = await fetch(
    `https://api.github.com/repos/ackrate/ackrate-protocol-contracts/commits/${encodeURIComponent(ref)}`,
    { cache: "no-store" },
  );
  if (!commitResponse.ok) throw new Error(`GitHub source resolution returned HTTP ${commitResponse.status}.`);
  const commit = await commitResponse.json();
  if (!/^[0-9a-f]{40}$/.test(commit.sha)) throw new Error("GitHub did not return an immutable source commit.");
  // Fetch by immutable SHA, never a mutable branch. This avoids stale/negative
  // raw.githubusercontent.com branch caches and binds the bytes to the resolved ref.
  const wasmUrl = `https://raw.githubusercontent.com/ackrate/ackrate-protocol-contracts/${commit.sha}/web/public/mandate_registry.wasm`;
  const response = await fetch(wasmUrl, { cache: "no-store" });
  if (!response.ok) throw new Error(`GitHub WASM download returned HTTP ${response.status}.`);
  const wasm = new Uint8Array(await response.arrayBuffer());
  const hash = new Uint8Array(await crypto.subtle.digest("SHA-256", wasm));
  const hashHex = toHex(hash);
  if (ref === "v3mainnet" && hashHex !== EXPECTED_WASM_HASH) {
    throw new Error("Latest branch WASM does not match the reviewed SHA-256.");
  }
  const attestationResponse = await fetch(
    `https://api.github.com/repos/ackrate/ackrate-protocol-contracts/attestations/sha256:${hashHex}`,
    { cache: "no-store" },
  );
  if (!attestationResponse.ok) throw new Error(`GitHub attestation lookup returned HTTP ${attestationResponse.status}.`);
  const attestationSet = await attestationResponse.json();
  const provenance = attestationSet.attestations?.map((attestation) => {
    try {
      return JSON.parse(atob(attestation.bundle.dsseEnvelope.payload));
    } catch {
      return null;
    }
  }).find((statement) => {
    const dependency = statement?.predicate?.buildDefinition?.resolvedDependencies?.[0];
    return statement?.subject?.some((subject) => subject.digest?.sha256 === hashHex)
      && statement?.predicate?.runDetails?.builder?.id === TRUSTED_BUILDER
      && dependency?.uri?.startsWith("git+https://github.com/ackrate/ackrate-protocol-contracts@")
      && /^[0-9a-f]{40}$/.test(dependency?.digest?.gitCommit);
  });
  if (!provenance) throw new Error("WASM has no trusted Ackrate GitHub build attestation; deployment refused.");
  const sourceCommit = provenance.predicate.buildDefinition.resolvedDependencies[0].digest.gitCommit;
  return {
    wasm,
    hash,
    ref,
    wasmUrl,
    requestedCommit: commit.sha,
    sourceCommit,
    sourceUrl: `https://github.com/ackrate/ackrate-protocol-contracts/tree/${sourceCommit}/contracts/simple/mandate-registry`,
    attestationUrl: `https://github.com/ackrate/ackrate-protocol-contracts/attestations?query=subject-digest%3A${hashHex}`,
  };
}

function showArtifact(artifact) {
  byId("artifact-commit").textContent = artifact.requestedCommit;
  byId("artifact-hash").textContent = toHex(artifact.hash);
  byId("artifact-source").href = artifact.sourceUrl;
  byId("artifact-source").textContent = artifact.sourceUrl;
  byId("artifact-result").classList.remove("hidden");
}

async function readContractState(contractId) {
  const ledgerKey = xdr.LedgerKey.contractData(new xdr.LedgerKeyContractData({
    contract: new Address(contractId).toScAddress(),
    key: xdr.ScVal.scvLedgerKeyContractInstance(),
    durability: xdr.ContractDataDurability.persistent(),
  }));
  const response = await rpcServer().getLedgerEntries(ledgerKey);
  if (response.entries.length !== 1) throw new Error(`Contract was not found on ${network().label}.`);
  const storage = response.entries[0].val.contractData().val().instance().storage() || [];
  const values = new Map(storage.map((entry) => {
    const key = scValToNative(entry.key());
    return [Array.isArray(key) ? key[0] : key, scValToNative(entry.val())];
  }));
  if (!values.has("Admin")) throw new Error("Contract instance has no Admin storage entry.");
  return {
    admin: String(values.get("Admin")),
    paused: values.get("Paused") ?? false,
  };
}

async function appendFreighterSignature() {
  if (xdrInput.value.trim() !== inspectedXdr) throw new Error("XDR changed after inspection; inspect it again.");
  const wallet = await connectedWallet();
  const transaction = inspect(inspectedXdr);
  const policy = assertTwoOfThree(await readAccountPolicy(transaction.source));
  const eligible = [policy.account, ...policy.signers.map((signer) => signer.key)];
  if (!eligible.includes(wallet)) throw new Error("Connected Freighter key is not a signer on this admin account.");
  const walletHint = toHex(Keypair.fromPublicKey(wallet).signatureHint());
  if (transaction.signatures.some((signature) => toHex(signature.hint()) === walletHint)) {
    throw new Error("This signer appears to have already signed the envelope.");
  }
  setStatus(`Waiting for ${wallet} to approve this exact transaction in Freighter…`);
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
  setStatus(`Signature appended by ${wallet}. Copy this new URL for the next cosigner.`);
  return wallet;
}

byId("environment").addEventListener("change", () => {
  setNetworkUi();
  inspectedXdr = "";
  signButton.disabled = true;
  setStatus(`Selected ${network().label}. Check Freighter is on the same network.`);
});

byId("check-network").addEventListener("click", async () => {
  try {
    await assertNetwork();
    setStatus(`${network().label} Horizon and Stellar RPC are reachable and agree on the expected network.`);
  } catch (error) {
    setStatus(`Network check failed: ${error.message}`);
  }
});

byId("fund-testnet").addEventListener("click", async () => {
  try {
    if (byId("environment").value !== "testnet") throw new Error("Faucet funding is testnet-only.");
    await assertNetwork();
    const wallet = await connectedWallet();
    setStatus(`Requesting testnet faucet funds for ${wallet}…`);
    const funding = await rpcServer().fundAddress(wallet);
    setStatus(`Testnet wallet funded: ${wallet}.`, funding.txHash);
  } catch (error) {
    setStatus(`Testnet funding failed: ${error.message}`);
  }
});

byId("verify-artifact").addEventListener("click", async () => {
  try {
    setStatus("Resolving the Git ref and verifying its GitHub build attestation…");
    const artifact = await reviewedWasm();
    showArtifact(artifact);
    setStatus(`Artifact verified: ${toHex(artifact.hash)} from attested source ${artifact.sourceCommit}.`);
  } catch (error) {
    setStatus(`Artifact verification failed: ${error.message}`);
  }
});

byId("read-policy").addEventListener("click", async () => {
  try {
    setStatus("Reading the account signer policy from Stellar RPC…");
    await assertNetwork();
    const policy = await readAccountPolicy(byId("policy-account").value.trim());
    byId("policy-result").textContent = JSON.stringify(policy, null, 2);
    try {
      assertTwoOfThree(policy);
      setStatus("RPC confirms an exact 2-of-3 account policy.");
      byId("admin").value = policy.account;
      byId("deploy-admin").value = policy.account;
      persistPublicIds();
    } catch (error) {
      setStatus(`Policy loaded, but not ready: ${error.message}`);
    }
  } catch (error) {
    setStatus(`Policy read failed: ${error.message}`);
  }
});

byId("configure-policy").addEventListener("click", async () => {
  try {
    setStatus("Checking the network and account before opening Freighter…");
    await assertNetwork();
    const account = byId("policy-account").value.trim();
    const signer2 = byId("signer-2").value.trim();
    const signer3 = byId("signer-3").value.trim();
    const keys = [account, signer2, signer3];
    if (!keys.every((key) => StrKey.isValidEd25519PublicKey(key))) throw new Error("All three entries must be valid G-accounts.");
    if (new Set(keys).size !== 3) throw new Error("All three signer keys must be different.");
    const wallet = await connectedWallet();
    if (wallet !== account) throw new Error("Freighter must be connected as the G-account being configured.");
    const before = await readAccountPolicy(account);
    if (before.signers.length !== 0 || before.masterWeight !== 1) {
      throw new Error("For safety, automatic setup only accepts a default account with master weight 1 and no additional signers.");
    }
    const result = await freighterClassicTransaction([
      Operation.setOptions({ signer: { ed25519PublicKey: signer2, weight: 1 } }),
      Operation.setOptions({ signer: { ed25519PublicKey: signer3, weight: 1 } }),
      Operation.setOptions({ masterWeight: 1, lowThreshold: 2, medThreshold: 2, highThreshold: 2 }),
    ], wallet, "Approve the atomic 2-of-3 account policy in Freighter…");
    const policy = assertTwoOfThree(await readAccountPolicy(account));
    byId("policy-result").textContent = JSON.stringify({ ...policy, transaction: result.txHash }, null, 2);
    setTransactionLink("policy-transaction", result.txHash);
    byId("policy-transaction-row").classList.remove("hidden");
    byId("admin").value = account;
    byId("deploy-admin").value = account;
    persistPublicIds();
    setStatus("2-of-3 account configured and independently verified through RPC.", result.txHash);
  } catch (error) {
    setStatus(`2-of-3 setup failed: ${error.message}`);
  }
});

byId("deploy").addEventListener("click", async () => {
  try {
    setStatus("Checking network, wallet, artifact, and future admin before deployment…");
    const admin = byId("deploy-admin").value.trim();
    if (!StrKey.isValidEd25519PublicKey(admin)) throw new Error("Future admin must be a valid G-account.");
    const selected = await assertNetwork();
    const wallet = await connectedWallet();
    const accountResponse = await fetch(`${selected.horizon}/accounts/${wallet}`);
    if (!accountResponse.ok) throw new Error(`Freighter account is not funded on ${selected.label}.`);
    const artifact = await reviewedWasm();
    showArtifact(artifact);
    const { wasm, hash, sourceUrl, attestationUrl } = artifact;

    const upload = await freighterTransaction(
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
    byId("deployed-source").href = sourceUrl;
    byId("deployed-source").textContent = sourceUrl;
    byId("deployed-attestation").href = attestationUrl;
    byId("deployed-attestation").textContent = "GitHub build attestations for this WASM hash";
    setTransactionLink("upload-tx", upload.txHash);
    setTransactionLink("deployment-tx", deployment.txHash);
    byId("deployment-result").classList.remove("hidden");
    byId("contract-input").value = contractId;
    byId("admin").value = admin;
    byId("hash-input").value = toHex(hash);
    persistPublicIds();
    setStatus(`Initial SimpleContract deployed on ${selected.label}: ${contractId}.`, deployment.txHash);
  } catch (error) {
    setStatus(`Deployment failed: ${error.message}`);
  }
});

byId("admin-operation").addEventListener("change", () => {
  const action = byId("admin-operation").value;
  byId("upgrade-hash-field").classList.toggle("hidden", action !== "upgrade");
  byId("use-git-wasm").classList.toggle("hidden", action !== "upgrade");
  byId("new-admin-field").classList.toggle("hidden", action !== "set_admin");
});

byId("use-git-wasm").addEventListener("click", async () => {
  try {
    setStatus("Checking network, wallet, GitHub artifact, and attestation before upload…");
    await assertNetwork();
    const wallet = await connectedWallet();
    const artifact = await reviewedWasm();
    showArtifact(artifact);
    const { wasm, hash, ref, sourceUrl, attestationUrl } = artifact;
    const result = await freighterTransaction(
      Operation.uploadContractWasm({ wasm }),
      wallet,
      `Approve the ${ref} WASM upload in Freighter…`,
    );
    byId("hash-input").value = toHex(hash);
    persistPublicIds();
    setStatus(`WASM from ${ref} uploaded. Hash ${toHex(hash)}. Source ${sourceUrl}. Attestation ${attestationUrl}.`, result.txHash);
  } catch (error) {
    setStatus(`WASM upload failed: ${error.message}`);
  }
});

byId("read-contract").addEventListener("click", async () => {
  try {
    setStatus("Reading contract admin and pause state through RPC simulation…");
    await assertNetwork();
    const contractId = byId("contract-input").value.trim();
    if (!StrKey.isValidContract(contractId)) throw new Error("Enter a valid contract ID.");
    const state = await readContractState(contractId);
    byId("contract-state").textContent = JSON.stringify(state, null, 2);
    setStatus(`Contract admin and pause state read directly from RPC ledger state. Admin: ${state.admin}.`);
  } catch (error) {
    setStatus(`Contract read failed: ${error.message}`);
  }
});

byId("prepare").addEventListener("click", async () => {
  try {
    setStatus("Checking network and on-chain 2-of-3 policy before simulation…");
    const signer = byId("admin").value.trim();
    const contractId = byId("contract-input").value.trim();
    const action = byId("admin-operation").value;
    if (!StrKey.isValidEd25519PublicKey(signer)) throw new Error("Signer must be a valid G-account.");
    if (!StrKey.isValidContract(contractId)) throw new Error("Contract must be a valid C-address.");
    await assertNetwork();
    setStatus("Reading the authoritative admin from the contract…");
    const state = await readContractState(contractId);
    const admin = state.admin;
    if (!StrKey.isValidEd25519PublicKey(admin)) throw new Error("Contract admin is not a G-account.");
    const policy = assertTwoOfThree(await readAccountPolicy(admin));
    const eligible = [policy.account, ...policy.signers.map((entry) => entry.key)];
    if (!eligible.includes(signer)) throw new Error("Entered signer is not part of the contract admin's 2-of-3 policy.");
    byId("policy-account").value = admin;
    byId("policy-result").textContent = JSON.stringify(policy, null, 2);
    setStatus(`Building ${action} for the verified 2-of-3 admin…`);
    const server = rpcServer();
    const account = await server.getAccount(admin);
    const contract = new Contract(contractId);
    let operation;
    if (action === "upgrade") {
      operation = contract.call("upgrade", nativeToScVal(fromHex(byId("hash-input").value), { type: "bytes" }));
    } else if (action === "set_admin") {
      const newAdmin = byId("new-admin").value.trim();
      if (!StrKey.isValidEd25519PublicKey(newAdmin)) throw new Error("New admin must be a valid G-account.");
      operation = contract.call("set_admin", new Address(newAdmin).toScVal());
    } else if (action === "pause" || action === "unpause") {
      operation = contract.call(action);
    } else {
      throw new Error("Unsupported admin operation.");
    }
    const transaction = new TransactionBuilder(account, {
      fee: "100",
      networkPassphrase: network().passphrase,
    }).addOperation(operation).setTimeout(900).build();
    const prepared = await server.prepareTransaction(transaction);
    const xdr = prepared.toXDR();
    xdrInput.value = xdr;
    inspect(xdr);
    history.replaceState(null, "", makeShareUrl(xdr));
    setStatus(`${action} preparation succeeded. Opening Freighter for the first signature…`);
    try {
      await appendFreighterSignature();
    } catch (signingError) {
      setStatus(`${action} is prepared, but the first signature was not added: ${signingError.message} You can retry with “Append Freighter signature”.`);
    }
  } catch (error) {
    setStatus(`Preparation failed: ${error.message}`);
  }
});

byId("inspect").addEventListener("click", () => {
  try {
    inspect(xdrInput.value);
    const method = byId("function").textContent;
    setStatus(`Decoded as one ${network().label} SimpleContract ${method} operation. Verify every field before signing.`);
  } catch (error) {
    inspectedXdr = "";
    signButton.disabled = true;
    byId("review").classList.add("hidden");
    byId("share").classList.add("hidden");
    setStatus(`Invalid admin transaction: ${error.message}`);
  }
});

signButton.addEventListener("click", async () => {
  try {
    setStatus("Checking the connected Freighter signer and current account policy…");
    await appendFreighterSignature();
  } catch (error) {
    setStatus(`Signing failed: ${error.message}`);
  }
});

byId("copy-url").addEventListener("click", async () => {
  await navigator.clipboard.writeText(byId("share-url").value);
  setStatus("Cosigner URL copied. It contains the network, transaction, and signatures in its fragment.");
});

byId("submit").addEventListener("click", async () => {
  try {
    const transaction = inspect(xdrInput.value);
    if (transaction.signatures.length < 2) throw new Error("Two signatures are required before submission.");
    await assertNetwork();
    setStatus("Submitting the signed envelope through RPC…");
    const sent = await rpcServer().sendTransaction(transaction);
    if (sent.status === "ERROR") throw new Error(sent.errorResult?.toString() || "RPC rejected the transaction.");
    const txHash = sent.hash || toHex(transaction.hash());
    const result = await waitForTransaction(rpcServer(), txHash);
    setStatus(`Admin operation confirmed on ${network().label}.`, result.txHash);
  } catch (error) {
    setStatus(`Submission failed: ${error.message}`);
  }
});

restorePublicIds();
setNetworkUi();
try {
  loadShareUrl();
} catch (error) {
  setStatus(`Could not load cosigner URL: ${error.message}`);
}
