#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFile, stat } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const manifestPath = resolve(root, "contracts/mainnet/deployment-manifest.template.json");
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

const source = objectAt(manifest, "source");
if (textAt(source, "branch") !== "main") {
  throw new Error("candidate manifest source branch must be main");
}
if (textAt(source, "build_platform") !== "ubuntu-24.04-x86_64") {
  throw new Error("candidate manifest build platform must be ubuntu-24.04-x86_64");
}
if (textAt(source, "rust_toolchain_version") !== "1.98.0") {
  throw new Error("candidate manifest Rust toolchain version must be 1.98.0");
}
if (textAt(source, "stellar_cli_version") !== "27.0.0") {
  throw new Error("candidate manifest Stellar CLI version must be 27.0.0");
}

const artifacts = objectAt(manifest, "artifacts");
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
