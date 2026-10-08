export function feeStroops(text) {
  const match = /^([0-9]+)(?:\.([0-9]{1,7}))?$/.exec(text.trim());
  if (!match) throw new Error("Enter a decimal XLM fee cap with at most seven decimal places");
  const stroops = BigInt(match[1]) * 10000000n + BigInt((match[2] ?? "").padEnd(7, "0"));
  if (stroops < 10000n || stroops > 5000000000n) throw new Error("Fee cap must be between 0.001 and 500 XLM");
  return stroops.toString();
}
export function pendingSubmissions(storage) {
  const records = [];
  for (let i = 0; i < storage.length; i++) {
    const key = storage.key(i);
    if (key.startsWith("ackrate-transfer:") && key.endsWith(":submitted")) records.push({ key, ...JSON.parse(storage.getItem(key)) });
  }
  return records;
}
export function expiredUnconsumed(record, result, sourceSequence) {
  return result.status === "NOT_FOUND" && Number(result.latestLedgerCloseTime) > Number(record.maxTime) && record.sequence !== undefined && BigInt(sourceSequence) < BigInt(record.sequence);
}
