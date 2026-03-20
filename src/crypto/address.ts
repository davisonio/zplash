import { bech32 } from 'bech32';
import initWasm, { derive_address_bytes } from '../../wasm-sweep/pkg/zplash_wasm.js';

let ready = false;
let loading: Promise<void> | null = null;

async function ensureWasm() {
  if (ready) return;
  if (!loading) {
    loading = initWasm().then(() => { ready = true; });
  }
  await loading;
}

/**
 * Derive a real Sapling payment address from a spending key.
 * Uses sapling-crypto compiled to WASM for Jubjub curve operations.
 */
export async function deriveSaplingAddress(
  spendingKey: Uint8Array,
  testnet = false
): Promise<string> {
  await ensureWasm();
  const addressBytes = derive_address_bytes(spendingKey);
  const words = bech32.toWords(addressBytes);
  const hrp = testnet ? 'ztestsapling' : 'zs';
  return bech32.encode(hrp, words, 120);
}
