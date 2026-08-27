#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const PUBLIC_PASSPHRASE = "Public Global Stellar Network ; September 2015";
const USDC_ISSUER = "GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN";
const USDC_SAC = "CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75";
const MIN_DELAY_LEDGERS = 17_280;
const ACCOUNT = /^G[A-Z2-7]{55}$/;

function required(name) {
  const value = process.env[name];
  if (!value) throw new Error(`missing ${name}`);
  return value;
}

function publicHttps(name, fallback) {
  const raw = process.env[name] || fallback;
  const url = new URL(raw);
  if (
    url.protocol !== "https:"
    || url.username
    || url.password
    || url.search
    || url.hash
  ) {
    throw new Error(`${name} must be a credential-free HTTPS URL without query or fragment`);
  }
  return url;
}

function account(name) {
  const value = required(name);
  if (!ACCOUNT.test(value)) throw new Error(`${name} must be a public Stellar G-account`);
  return value;
}

function exactObject(value, keys, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  if (actual.length !== expected.length || actual.some((key, index) => key !== expected[index])) {
    throw new Error(`${label} schema is invalid`);
  }
}

async function getJson(url, label) {
  const response = await fetch(url, { headers: { accept: "application/json" } });
  if (!response.ok) throw new Error(`${label} returned HTTP ${response.status}`);
  return response.json();
}

async function checkRpc(rpcUrl) {
  const headers = { "content-type": "application/json" };
  const rpcHeader = process.env.ACKRATE_MAINNET_RPC_HEADER;
  if (rpcHeader) {
    const separator = rpcHeader.indexOf(":");
    if (separator <= 0 || separator === rpcHeader.length - 1) {
      throw new Error("ACKRATE_MAINNET_RPC_HEADER must use the form 'Header-Name: value'");
    }
    const name = rpcHeader.slice(0, separator).trim();
    const value = rpcHeader.slice(separator + 1).trim();
    if (!/^[A-Za-z0-9-]+$/.test(name) || !value || /[\r\n]/.test(value)) {
      throw new Error("ACKRATE_MAINNET_RPC_HEADER is invalid");
    }
    headers[name] = value;
  }
  const response = await fetch(rpcUrl, {
    method: "POST",
    headers,
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "getHealth" }),
  });
  if (!response.ok) throw new Error(`mainnet RPC health returned HTTP ${response.status}`);
  const body = await response.json();
  if (body?.result?.status !== "healthy") throw new Error("mainnet RPC did not report healthy");
}

function assertTwoOfThree(accountRecord, expectedSigners) {
  const signers = accountRecord.signers;
  if (!Array.isArray(signers) || signers.length !== 3) {
    throw new Error("authority account must have exactly three signers and no alternate signer types");
  }
  const observed = new Map();
  for (const signer of signers) {
    if (signer.type !== "ed25519_public_key" || !ACCOUNT.test(signer.key)) {
      throw new Error("authority account contains an unsupported signer type");
    }
    observed.set(signer.key, signer.weight);
  }
  if (observed.size !== 3 || expectedSigners.some((key) => !observed.has(key))) {
    throw new Error("authority account signer set differs from the authority manifest");
  }

  const low = accountRecord.thresholds?.low_threshold;
  const medium = accountRecord.thresholds?.med_threshold;
  const high = accountRecord.thresholds?.high_threshold;
  if (low !== 2 || medium !== 2 || high !== 2) {
    throw new Error("authority account low, medium, and high thresholds must each equal 2");
  }

  const weights = expectedSigners.map((key) => observed.get(key));
  for (const weight of weights) {
    if (weight !== 1) {
      throw new Error("each authority signer must have weight 1");
    }
  }
  for (let left = 0; left < weights.length; left += 1) {
    for (let right = left + 1; right < weights.length; right += 1) {
      if (weights[left] + weights[right] < medium || weights[left] + weights[right] < high) {
        throw new Error("every authority signer pair must satisfy medium and high thresholds");
      }
    }
  }
}

const authority = account("ACKRATE_AUTHORITY_2_OF_3");
const pauser = account("ACKRATE_EMERGENCY_PAUSER");
const deploymentSource = account("ACKRATE_DEPLOYMENT_SOURCE_ACCOUNT");
if (pauser === authority) throw new Error("emergency pauser must be separate from the 2-of-3 authority");

if (required("ACKRATE_MAINNET_USDC_SAC") !== USDC_SAC) {
  throw new Error("ACKRATE_MAINNET_USDC_SAC does not match the independently derived Circle USDC SAC");
}
if (required("ACKRATE_MAINNET_NETWORK_PASSPHRASE") !== PUBLIC_PASSPHRASE) {
  throw new Error("ACKRATE_MAINNET_NETWORK_PASSPHRASE is not the Stellar Public Network passphrase");
}
const delay = Number(required("ACKRATE_TIMELOCK_DELAY_LEDGERS"));
if (!Number.isInteger(delay) || delay < MIN_DELAY_LEDGERS || delay > 0xffff_ffff) {
  throw new Error(`ACKRATE_TIMELOCK_DELAY_LEDGERS must be between ${MIN_DELAY_LEDGERS} and 4294967295`);
}

const authorityManifest = JSON.parse(
  await readFile(required("ACKRATE_AUTHORITY_MANIFEST"), "utf8"),
);
exactObject(
  authorityManifest,
  ["version", "network", "authorityAccount", "requiredSignatures", "signers"],
  "authority manifest",
);
if (
  authorityManifest.version !== 1
  || authorityManifest.network !== "mainnet"
  || authorityManifest.authorityAccount !== authority
  || authorityManifest.requiredSignatures !== 2
  || !Array.isArray(authorityManifest.signers)
  || authorityManifest.signers.length !== 3
) {
  throw new Error("authority manifest does not describe the selected mainnet 2-of-3 account");
}
const labels = new Set();
const publicKeys = new Set();
for (const signer of authorityManifest.signers) {
  exactObject(signer, ["label", "publicKey"], "authority signer");
  if (!["A", "B", "C"].includes(signer.label) || !ACCOUNT.test(signer.publicKey)) {
    throw new Error("authority signer label or public key is invalid");
  }
  labels.add(signer.label);
  publicKeys.add(signer.publicKey);
}
if (labels.size !== 3 || publicKeys.size !== 3 || !publicKeys.has(authority)) {
  throw new Error("authority manifest must contain unique A/B/C signers including the account master key");
}

const horizonUrl = publicHttps("ACKRATE_MAINNET_HORIZON_URL", "https://horizon.stellar.org");
const rpcUrl = publicHttps("ACKRATE_MAINNET_RPC_URL");
const [authorityRecord, sourceRecord, assets] = await Promise.all([
  getJson(new URL(`/accounts/${authority}`, horizonUrl), "authority account lookup"),
  getJson(new URL(`/accounts/${deploymentSource}`, horizonUrl), "deployment source lookup"),
  getJson(
    new URL(`/assets?asset_code=USDC&asset_issuer=${USDC_ISSUER}&limit=1`, horizonUrl),
    "USDC asset lookup",
  ),
  checkRpc(rpcUrl),
]);

assertTwoOfThree(authorityRecord, [...publicKeys]);
const nativeBalance = sourceRecord.balances?.find((balance) => balance.asset_type === "native");
if (!nativeBalance || Number(nativeBalance.balance) <= 0) {
  throw new Error("deployment source account has no native XLM balance for fees and rent");
}
if (!Array.isArray(assets?._embedded?.records) || assets._embedded.records.length !== 1) {
  throw new Error("Circle Stellar mainnet USDC asset was not found through Horizon");
}

process.stdout.write("Mainnet preflight passed.\n");
process.stdout.write(`Authority: ${authority} (verified on-chain 2-of-3 signer math and thresholds)\n`);
process.stdout.write("Independent physical custody handoff: not asserted by this preflight\n");
process.stdout.write(`Deployment source: ${deploymentSource} (${nativeBalance.balance} XLM)\n`);
process.stdout.write(`Circle USDC SAC: ${USDC_SAC}\n`);
process.stdout.write(`Timelock minimum: ${delay} ledgers\n`);
