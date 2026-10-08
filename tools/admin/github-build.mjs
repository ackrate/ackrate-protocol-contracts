import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { ADMIN_WASM_HASHES, ADMIN_WASM_FILES } from "./release.mjs";
const exec = promisify(execFile);
export const REPOSITORY = "ackrate/ackrate-protocol-contracts";
export function verifyRun(run, jobs, commit) {
  if (!/^[0-9a-f]{40}$/.test(commit) || run.head_sha !== commit || run.status !== "completed" || run.conclusion !== "success" || run.name !== "Contract Gate Check" || run.repository.full_name !== REPOSITORY || run.head_repository?.full_name !== REPOSITORY || run.path !== ".github/workflows/ci.yml" || !["push", "pull_request"].includes(run.event)) throw new Error("Build must be a successful Contract Gate Check for the exact selected source commit");
  for (const name of ["security", "fixed-target administration"]) {
    const job = jobs.find((j) => j.name === name);
    if (!job || job.status !== "completed" || job.conclusion !== "success") throw new Error(`Required ${name} gate did not pass`);
  }
}
export function verifyBuild(build, commit, files) {
  if (build.version !== 1 || build.repository !== REPOSITORY || build.commit !== commit || build.rust !== "1.98.0" || build.stellar !== "28.1.0") throw new Error("Artifact provenance differs from selected build");
  for (const [name, expected] of Object.entries(ADMIN_WASM_HASHES)) {
    if (build.hashes[name] !== expected || createHash("sha256").update(files[name]).digest("hex") !== expected) throw new Error(`${name} artifact differs from reviewed WASM pin`);
  }
}
export async function githubBuild(runId, commit) {
  if (!/^[1-9][0-9]*$/.test(runId ?? "") || !/^[0-9a-f]{40}$/.test(commit ?? "")) throw new Error("Explicit GitHub build run and reviewed source commit required");
  const api = async (path) => JSON.parse((await exec("gh", ["api", `repos/${REPOSITORY}/${path}`], { maxBuffer: 4000000 })).stdout);
  const run = await api(`actions/runs/${runId}`);
  const { jobs } = await api(`actions/runs/${runId}/jobs?per_page=100`);
  verifyRun(run, jobs, commit);
  const name = `admin-contracts-${commit}`;
  const { artifacts } = await api(`actions/runs/${runId}/artifacts?per_page=100`);
  if (artifacts.filter((a) => a.name === name && !a.expired).length !== 1) throw new Error("Exact unexpired admin build artifact unavailable");
  const directory = await mkdtemp(join(tmpdir(), "ackrate-admin-build-"));
  try {
    await exec("gh", ["run", "download", runId, "--repo", REPOSITORY, "--name", name, "--dir", directory], { maxBuffer: 4000000 });
    const build = JSON.parse(await readFile(join(directory, "build.json"), "utf8"));
    const files = Object.fromEntries(await Promise.all(Object.keys(ADMIN_WASM_HASHES).map(async (key) => [key, await readFile(join(directory, ADMIN_WASM_FILES[key]))])));
    verifyBuild(build, commit, files);
    return { build: { ...build, runId, runUrl: run.html_url }, files };
  } finally { await rm(directory, { recursive: true, force: true }); }
}
