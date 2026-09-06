# Mainnet V2 security scan and findings record

**Verification date:** 2026-09-07 (Bangkok, UTC+7).
**Scope:** V2 deployed contract security, test sensitivity, dependency resolution,
and continuous release gates. The historical `security-scan-report.md` is not the
current V2 result. No deployed runtime code was changed by this review.

## Fresh executable results

The full local gate passed at **2026-09-06 23:25:28–23:25:57 UTC** after
strengthening the authorization tests. A final complete rerun, including the
hostile-agent fixture and all 13 scanner scenarios, passed by **23:41:42 UTC**
(**2026-09-07 06:41:42 Bangkok**). It includes formatting, warnings-denied lint, the exact required-test
manifest, offline deployment and scanner regressions, artifact size, and interface.

| Suite | Distinct executable tests |
|---|---:|
| Deployed V2 native host | 52 |
| Deployed V2 optimized-WASM smoke | 1 |
| Simple development variant | 32 |
| Composite development variant | 64 |
| Historical mainnet Registry | 23 |
| Historical TimelockController | 11 |
| Repository total | 183 |

The gate executes V2 native tests once normally and again with the optimized-WASM
feature, so the repository run contains **235 executions, not 235 distinct tests**.
The other variants are regression coverage, not additional tests of deployed V2.
The 13 scanner regression scenarios are separate shell scenarios, not Rust tests.

## Dependency scan

The final hardened scan passed at **2026-09-06 23:37:18–23:37:23 UTC**
(**2026-09-07 06:37:18–06:37:23 Bangkok**).
It used the pinned dependency scanner `0.22.2`, Rust `1.98.0`, and RustSec database
commit `5a0ebedfe8bdd2e295b171f4162f8c977bcad9a5` (1,239 advisories;
database update 2026-09-02). Results are time-bound to that advisory database.

| Build | Actual resolved lockfile | Dependencies scanned | Result |
|---|---|---:|---|
| Deployed V2 workspace | Root `Cargo.lock` | 188 | No known vulnerabilities, no yanked crates, no unexpected warnings |
| Historical Registry | `contracts/mainnet/mandate-registry/Cargo.lock` | 192 | Same policy passed |
| Historical TimelockController | `contracts/mainnet/timelock-controller/Cargo.lock` | 197 | Same policy passed |

Each complete normal-dependency graph for `wasm32v1-none` resolved successfully.
The existing accepted advisory `RUSTSEC-2024-0436` (`paste`, unmaintained) was absent
from all three deployed-target graphs. This is a **host/test-only maintenance
exception**, not a patched upstream dependency and not proof that the build host
has no supply-chain exposure. The policy permits that exact advisory only; other
warnings fail. Revisit the exception when a compatible dependency update removes it.

## Findings and disposition

| ID | Finding | Disposition and regression evidence |
|---|---|---|
| S2-01 | A failed inverse dependency lookup could be mistaken for proof that the accepted advisory was absent from deployed WASM | Fixed: require successful, nonempty resolution of the full deployed graph. Regression rejects command failure, partial-output failure, empty graph, and actual `paste` inclusion |
| S2-02 | The V2 scan selected a nested lockfile even though Cargo builds this member using the root workspace lock | Fixed: resolve Cargo's workspace and scan its actual lockfile. The two files were identical at discovery, so no present divergent dependency was found. Regression detects wrong-lock selection and failed/empty workspace discovery |
| S2-03 | Unauthorized upgrade and asset-policy negative tests could be satisfied by an unrelated unpaused-state rejection | Fixed: pause through the legitimate administrator and upload valid replacement WASM before testing missing/wrong authorization; assert rejection with preserved state/events. Mutation sensitivity is recorded in the verification evidence |
| S2-04 | Unknown-mandate revocation and schema getter behavior were not precisely represented in the coverage table | Fixed: extend the existing unknown-ID test with revocation/no-event/unchanged-state checks. Document that the schema getter returns an existing predecessor value while mandate methods reject it |
| S2-05 | Standalone threat-model/data-flow/scan files and some hosted reproduction commands referred to legacy or incomplete workflows | Current V2 model, entity diagrams, and this scan record published separately; current entry points link these. Hosted commands build the artifact required by all-feature tests and include mandatory read-only verification arguments |
| S2-06 | The pinned upstream scanner can return success after failing to retrieve registry/yanked-package metadata; user-level settings could also suppress checks or diagnostics | Fixed: repository-local scanner policy requires metadata fetching, yank checks, and visible terminal diagnostics. The wrapper preserves failure codes and rejects operational warnings/errors even when the upstream exit code is zero. Five additional regressions include the three actual upstream metadata failure messages |
| S2-07 | A hostile-agent overspend test used a token allowance equal to the mandate cap, so token rejection could mask a missing contract budget check | Fixed: fixture funds and registry allowance both exceed the forbidden purchase. All five hostile tests pass; removing the contract's upper-budget check now fails at the intended overspend assertion; restoring it passes again |
| E-01 | Existing `RUSTSEC-2024-0436` upstream maintenance advisory in host/test dependencies | Accepted, not remediated upstream. Exact exception plus enforced absence from deployed WASM; no blanket claim that every dependency finding has disappeared |

S2-01 through S2-03 are weaknesses in verification safeguards, not demonstrated
deployed payment exploits. They matter because a gate must reject failures for the
right reason. No deployed payment bypass was found in this review. That statement
does not imply unknown vulnerabilities are impossible.

The S2-03 mutation check ran against isolated source and build directories at
**2026-09-06 23:30 UTC**. The two strengthened tests passed on the unchanged
baseline, both failed when only upgrade authorization was removed, both failed
when only asset-policy authorization was removed, and both passed after the
original runtime was restored. Failures occurred at the intended authorization
assertions, with valid WASM and a properly paused registry. No mutation was
applied to deployed code or the repository's runtime source.

At **23:36 UTC**, a third isolated mutant removed only the `new_spent > max_amount`
rejection. The strengthened hostile-agent test failed because the forbidden
purchase succeeded; compilation, token balance, and allowance were valid. All
five hostile-agent tests passed before and after restoration. These are three
targeted mutant implementations, not exhaustive mutation testing.

## Reproduction and continuous enforcement

From the repository root, with the pinned tools installed:

```bash
bash scripts/test-security-scan.sh
./scripts/security-scan.sh
./scripts/gatecheck-contracts.sh
```

The scanner's thirteen offline scenarios cover clean resolution with the actual V2
workspace lock, forbidden dependency presence, failed graph resolution, partial
graph failure, empty graph, failed workspace discovery, empty workspace discovery,
failed advisory scanning, and five exit-zero operational warning/error scenarios.
Repository-local policy keeps metadata checking and diagnostics enabled even when
the caller starts outside the repository. A missing tool, failed dependency graph,
nonzero scanner result, or reported incomplete metadata is a failed check, never
a clean result. This is a check of the pinned scanner's modeled failure modes, not
a guarantee that an arbitrarily changed or compromised tool cannot lie.

[Continuous CI](../.github/workflows/ci.yml) runs the scanner scenarios and live
advisory scan before contract gates on pushes to `main` and pull requests. The
canonical Linux gate enforces the exact deployed WASM hash; a macOS build can have
different bytes and must not be substituted for that proof. See the
[verification report](mainnet-v2-security-verification.md) for the exact artifact,
coverage, independent reproduction, live read-only checks, and remaining risks.
