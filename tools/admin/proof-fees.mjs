export function proofFeeCap(network, testnetOverride) {
  if (!["mainnet", "testnet"].includes(network)) throw new Error("Unknown proof network");
  if (testnetOverride === undefined) return 500000000n;
  if (network !== "testnet") throw new Error("Proof fee override is Testnet-only");
  if (!/^[1-9][0-9]*$/.test(testnetOverride)) throw new Error("Testnet fee limit must be integer stroops");
  const cap = BigInt(testnetOverride);
  if (cap < 100n || cap > 2000000000n) throw new Error("Testnet fee limit must be between 100 stroops and 200 XLM");
  return cap;
}
