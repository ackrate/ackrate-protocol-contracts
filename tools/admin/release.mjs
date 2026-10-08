// Update these pins only after reviewing the exact build and final recipient.
export const HELPER_WASM_HASH = "de07eb3cf98bb4211cc35c943f4331864ead1354bfb55657e8b2cb92a4108bfb";
export const V2_WASM_HASH = "982809197d35d44c7b0fce6bd117fb2fec09b728c64c146c1f803b01faacff62";
export const PRODUCTION_GOVERNANCE = null;
export const ADMIN_WASM_HASHES = Object.freeze({
  timelock: HELPER_WASM_HASH,
  hello: "92933ea27374242a45c2f97c4a47ffa1631063b156ae3e89cdcb74afc79145a4",
});

// Retain approved historical hashes when a new immutable helper release changes code.
export const REVIEWED_HELPER_HASHES = Object.freeze([
  HELPER_WASM_HASH,
  "c36c03e9e7c138c1ced16e00159c0ce759ceffa9a48c90e00ac00e954b71413c",
]);

export const ADMIN_WASM_FILES = Object.freeze({ timelock: "ackrate_timelock_controller.wasm", hello: "ackrate_admin_hello.wasm" });
