import assert from "node:assert/strict";
import { test } from "node:test";
import { proofFeeCap } from "./proof-fees.mjs";

test("Mainnet proof retains its fixed cap and rejects every override", () => {
  assert.equal(proofFeeCap("mainnet"), 500000000n);
  for (const value of ["100", "500000000", "2000000000"]) assert.throws(() => proofFeeCap("mainnet", value), /Testnet-only/);
});
test("Testnet proof requires an explicit bounded integer override", () => {
  assert.equal(proofFeeCap("testnet"), 500000000n);
  assert.equal(proofFeeCap("testnet", "2000000000"), 2000000000n);
  for (const value of ["", "-1", "1.5", "1e9", "99", "2000000001"]) assert.throws(() => proofFeeCap("testnet", value));
  assert.throws(() => proofFeeCap("other", "100"), /Unknown/);
});
