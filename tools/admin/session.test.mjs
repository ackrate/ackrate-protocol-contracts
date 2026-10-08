import test from "node:test";
import assert from "node:assert/strict";
import { expiredUnconsumed, feeStroops, pendingSubmissions } from "./session.mjs";
test("decimal fees convert to stroops without floating-point rejection", () => {
  for (const [fee, expected] of [["0.035", "350000"], ["0.070", "700000"], ["0.085", "850000"], ["500.000", "5000000000"]]) assert.equal(feeStroops(fee), expected);
  assert.throws(() => feeStroops("500.001")); assert.throws(() => feeStroops("0.0001"));
});
test("pending records remain discoverable across networks and unloaded contexts", () => {
  const entries = new Map([["ackrate-transfer:mainnet:C1:submitted", '{"hash":"one"}'], ["ackrate-transfer:testnet:C2:submitted", '{"hash":"two"}'], ["ackrate-deployment:mainnet", '{}']]);
  const storage = { length: entries.size, key: (i) => [...entries.keys()][i], getItem: (key) => entries.get(key) };
  assert.deepEqual(pendingSubmissions(storage).map((r) => r.hash), ["one", "two"]);
});
test("a lagging RPC or consumed sequence cannot clear an uncertain submission", () => {
  const record = { maxTime: "100", sequence: "20" };
  assert(!expiredUnconsumed(record, { status: "NOT_FOUND", latestLedgerCloseTime: 90 }, "19"));
  assert(!expiredUnconsumed(record, { status: "NOT_FOUND", latestLedgerCloseTime: 101 }, "20"));
  assert(!expiredUnconsumed({ maxTime: "100" }, { status: "NOT_FOUND", latestLedgerCloseTime: 101 }, "19"));
  assert(expiredUnconsumed(record, { status: "NOT_FOUND", latestLedgerCloseTime: 101 }, "19"));
});
