# ACKRATE Timelock Controller

`governance account → TimelockController → managed contract`

The controller manages one contract through its existing V2 administrator API.
It uses OpenZeppelin Stellar `0.7.2` operation hashing, scheduling, cancellation,
predecessors and execution state. The native governance account enforces its own
signature threshold.

Constructor arguments are `target`, `governance` and `delay` in ledgers.
The constructor captures the target's current administrator. Target and delay
are fixed. Governance can transfer before adoption through `propose_governance`
and recipient-authorized `accept_governance`. Governance freezes after adoption.
The controller has no code-upgrade entry point.

Deployment and adoption use separate transactions:

1. Upload reviewed controller WASM if it is absent.
2. Deploy the controller with final governance.
3. Current target administration calls `propose_admin(controller)`.
4. Governance calls the controller's `accept_management()`.

A failed acceptance leaves the current target administrator unchanged. The pending
nomination persists from its earlier transaction. The current administrator can
replace that nomination before acceptance.

Governance authorizes `schedule`, `cancel` and immediate emergency `pause_now`.
The delayed `Action` values are `Upgrade(hash)`, `Unpause`, `AssetPolicy(asset, allowed)`
and `TransferAdmin(successor)`. Each operation binds the target, method, arguments,
predecessor and salt. Anyone can execute its exact payload after the delay.
Failed target calls roll back execution state. Completed actions cannot execute again.

Emergency pause does not invalidate queued actions. Cancel any ready unpause or
upgrade that must not proceed during an incident. Native account signer changes
remain native-account operations and do not pass through this controller's delay.

Replace an adopted controller by deploying a fresh instance for the same target,
scheduling `TransferAdmin(new-controller)` on the old controller, executing after
its delay, and accepting management on the new controller. The old controller
then loses its target authority. Paid storage rent has no refund.

Version `0.2.0` uses a new ABI and storage layout. The deployed `0.1.0` canary
controller remains recorded in `../deployment-manifest.json`. Its byte hashes and
release source are unchanged. It cannot upgrade itself to this version.


Run `scripts/gatecheck-contracts.sh` from the repository root. The controller tests
use reviewed Hello and deployed V2 WASM fixtures and the current controller build.
Deployment, signing and proof tooling live in the private project repository.
