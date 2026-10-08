import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { ADMIN_WASM_HASHES, ADMIN_WASM_FILES } from "./release.mjs";
const directory = process.argv[2];
if (!directory) throw new Error("Artifact directory required");
const commit = execFileSync("git", ["rev-parse", "HEAD"], { encoding: "utf8" }).trim();
await mkdir(directory, { recursive: true });
for (const [name, expected] of Object.entries(ADMIN_WASM_HASHES)) {
  const file = ADMIN_WASM_FILES[name];
  const source = new URL(`../../contracts/admin/target/wasm32v1-none/release/${file}`, import.meta.url);
  const bytes = await readFile(source);
  if (createHash("sha256").update(bytes).digest("hex") !== expected) throw new Error(`${name} differs from reviewed build`);
  await copyFile(source, `${directory}/${file}`);
}
await writeFile(`${directory}/build.json`, `${JSON.stringify({ version: 1, repository: "ackrate/ackrate-protocol-contracts", commit, rust: "1.98.0", stellar: "28.1.0", hashes: ADMIN_WASM_HASHES }, null, 2)}\n`);
