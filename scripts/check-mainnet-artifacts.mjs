#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFile, stat } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const manifestPath = resolve(root, "contracts/mainnet/deployment-manifest.template.json");
const completedManifestPath = resolve(root, "contracts/mainnet/deployment-manifest.json");
const ACCOUNT = /^G[A-Z2-7]{55}$/;
let releaseDir = resolve(root, "target/mainnet-release");
const releaseDirIndex = process.argv.indexOf("--release-dir");
if (releaseDirIndex >= 0) {
  const requestedDir = process.argv[releaseDirIndex + 1];
  if (!requestedDir) throw new Error("--release-dir requires a path");
  releaseDir = resolve(requestedDir);
}

function objectAt(parent, key) {
  const value = parent[key];
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`candidate manifest ${key} must be an object`);
  }
  return value;
}

function textAt(parent, key) {
  const value = parent[key];
  if (typeof value !== "string" || value.length === 0 || value.trim() !== value) {
    throw new Error(`candidate manifest ${key} must be a non-empty string`);
  }
  return value;
}

function sha256At(parent, key) {
  const value = textAt(parent, key);
  if (!/^[0-9a-f]{64}$/.test(value)) {
    throw new Error(`candidate manifest ${key} must be a lowercase SHA-256`);
  }
  return value;
}

function positiveIntegerAt(parent, key) {
  const value = parent[key];
  if (!Number.isSafeInteger(value) || value <= 0) {
    throw new Error(`candidate manifest ${key} must be a positive integer`);
  }
  return value;
}

async function digest(path) {
  return createHash("sha256").update(await readFile(path)).digest("hex");
}

const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
if (manifest.schema_version !== 1) {
  throw new Error("unsupported candidate manifest schema_version");
}
const completedManifest = JSON.parse(await readFile(completedManifestPath, "utf8"));
if (completedManifest.schema_version !== 1) {
  throw new Error("unsupported completed manifest schema_version");
}

const source = objectAt(manifest, "source");
if (textAt(source, "branch") !== "main") {
  throw new Error("candidate manifest source branch must be main");
}
if (textAt(source, "build_platform") !== "ubuntu-24.04-x86_64") {
  throw new Error("candidate manifest build platform must be ubuntu-24.04-x86_64");
}
if (textAt(source, "rust_toolchain_version") !== "1.96.0") {
  throw new Error("candidate manifest Rust toolchain version must be 1.96.0");
}
if (textAt(source, "stellar_cli_version") !== "27.0.0") {
  throw new Error("candidate manifest Stellar CLI version must be 27.0.0");
}

const candidateConfiguration = objectAt(manifest, "public_configuration");
if (candidateConfiguration.deployment_stage !== null) {
  throw new Error("candidate manifest deployment stage must remain unset before deployment");
}
if (textAt(candidateConfiguration, "authority_mode") !== "stellar_2_of_3") {
  throw new Error("candidate manifest authority mode must be stellar_2_of_3");
}
if (
  candidateConfiguration.final_independent_custody_handoff_complete !== false
  || candidateConfiguration.final_independent_custody_handoff_evidence !== null
) {
  throw new Error("candidate manifest must not pre-claim the independent custody handoff");
}

const completedSource = objectAt(completedManifest, "source");
if (textAt(completedSource, "commit") !== textAt(source, "commit")) {
  throw new Error("completed manifest source commit differs from the candidate manifest");
}

const completedConfiguration = objectAt(completedManifest, "public_configuration");
const deploymentStage = textAt(completedConfiguration, "deployment_stage");
if (!["mainnet_canary", "mainnet_final"].includes(deploymentStage)) {
  throw new Error("completed manifest deployment stage is invalid");
}
if (textAt(completedConfiguration, "authority_mode") !== "stellar_2_of_3") {
  throw new Error("completed manifest authority mode must be stellar_2_of_3");
}
const authorityAccount = textAt(completedConfiguration, "authority_2_of_3_account");
if (!ACCOUNT.test(authorityAccount)) {
  throw new Error("completed manifest authority_2_of_3_account is invalid");
}
const custodyComplete = completedConfiguration.final_independent_custody_handoff_complete;
const custodyEvidence = completedConfiguration.final_independent_custody_handoff_evidence;
if (typeof custodyComplete !== "boolean") {
  throw new Error("completed manifest custody handoff state must be boolean");
}
if (!custodyComplete && custodyEvidence !== null) {
  throw new Error("incomplete custody handoff must not publish completion evidence");
}
if (
  custodyComplete
  && (typeof custodyEvidence !== "string" || !custodyEvidence.trim())
) {
  throw new Error("completed custody handoff requires a non-empty evidence reference");
}
if (deploymentStage === "mainnet_final" && !custodyComplete) {
  throw new Error("mainnet_final requires the independent custody handoff to be complete");
}

const authority = objectAt(completedManifest, "authority");
const thresholds = objectAt(authority, "thresholds");
if (thresholds.low !== 2 || thresholds.medium !== 2 || thresholds.high !== 2) {
  throw new Error("completed manifest authority thresholds must all equal 2");
}
if (
  !Array.isArray(authority.signers)
  || authority.signers.length !== 3
  || new Set(authority.signers).size !== 3
  || authority.signers.some((signer) => !ACCOUNT.test(signer))
  || !authority.signers.includes(authorityAccount)
) {
  throw new Error("completed manifest must contain exactly three unique Ed25519 signer addresses");
}
if (
  !Array.isArray(authority.signer_weights)
  || authority.signer_weights.length !== 3
  || authority.signer_weights.some((weight) => weight !== 1)
) {
  throw new Error("completed manifest authority signer weights must all equal 1");
}

const constructorArguments = objectAt(completedManifest, "constructor_arguments");
const timelockConstructor = objectAt(constructorArguments, "timelock");
const registryConstructor = objectAt(constructorArguments, "mandate_registry");
if (
  !Array.isArray(timelockConstructor.proposers)
  || timelockConstructor.proposers.length !== 1
  || timelockConstructor.proposers[0] !== authorityAccount
  || registryConstructor.unpauser !== authorityAccount
) {
  throw new Error("completed manifest constructor authority does not match the 2-of-3 account");
}
const verification = objectAt(completedManifest, "verification");
if (
  verification.authority_is_proposer_and_canceller !== true
  || verification.registry_unpauser_is_2_of_3 !== true
) {
  throw new Error("completed manifest does not confirm the live 2-of-3 authority bindings");
}

const artifacts = objectAt(manifest, "artifacts");
const completedArtifacts = objectAt(completedManifest, "artifacts");
const candidates = [
  ["timelock_controller", "ackrate_timelock_controller.wasm"],
  ["mandate_registry", "mandate_registry.wasm"],
];
const hashes = [];

for (const [name, expectedFilename] of candidates) {
  const artifact = objectAt(artifacts, name);
  const expectedPath = `target/mainnet-release/${expectedFilename}`;
  if (textAt(artifact, "path") !== expectedPath) {
    throw new Error(`${name} artifact path must be ${expectedPath}`);
  }

  const expectedHash = sha256At(artifact, "sha256");
  const expectedSize = positiveIntegerAt(artifact, "size_bytes");
  const completedArtifact = objectAt(completedArtifacts, name);
  if (
    textAt(completedArtifact, "path") !== expectedPath
    || sha256At(completedArtifact, "sha256") !== expectedHash
    || positiveIntegerAt(completedArtifact, "size_bytes") !== expectedSize
  ) {
    throw new Error(`${name} completed-manifest artifact differs from the candidate manifest`);
  }
  const artifactPath = resolve(releaseDir, expectedFilename);
  const actualHash = await digest(artifactPath);
  const actualSize = (await stat(artifactPath)).size;

  if (actualHash !== expectedHash) {
    throw new Error(`${name} hash mismatch: expected ${expectedHash}, got ${actualHash}`);
  }
  if (actualSize !== expectedSize) {
    throw new Error(`${name} size mismatch: expected ${expectedSize}, got ${actualSize}`);
  }
  hashes.push(expectedHash);
}

if (process.argv.includes("--print-hashes")) {
  process.stdout.write(`${hashes.join(" ")}\n`);
} else {
  process.stdout.write("Reviewed mainnet artifact hashes and sizes match the candidate manifest.\n");
}
