import { bech32 } from 'bech32';
import { blake2bInit, blake2bUpdate, blake2bFinal } from 'blakejs';

const KEY_HRP = 'zkey';
const KEY_BYTES = 32;
const PERSONALIZATION = 'Zcash_PaymentURI'; // exactly 16 bytes

/**
 * Generate a cryptographically random 256-bit key.
 */
export function generateKey(): Uint8Array {
  return crypto.getRandomValues(new Uint8Array(KEY_BYTES));
}

/**
 * Derive a Sapling spending key using BLAKE2b-256 with Zcash payment URI personalization.
 *
 * The personalization is applied by XORing into the BLAKE2b parameter block
 * (bytes 48-63), which maps to state words h[6] and h[7], stored as
 * h[12..15] in blakejs's Uint32Array representation.
 */
export function deriveSpendingKey(key: Uint8Array): Uint8Array {
  if (key.length !== KEY_BYTES) {
    throw new Error(`Key must be ${KEY_BYTES} bytes, got ${key.length}`);
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const ctx: any = blake2bInit(32);

  // Apply personalization to the parameter block
  const personal = new TextEncoder().encode(PERSONALIZATION);
  const pBuf = new Uint8Array(16);
  pBuf.set(personal.subarray(0, 16));
  const pView = new DataView(pBuf.buffer);

  ctx.h[12] ^= pView.getUint32(0, true);
  ctx.h[13] ^= pView.getUint32(4, true);
  ctx.h[14] ^= pView.getUint32(8, true);
  ctx.h[15] ^= pView.getUint32(12, true);

  blake2bUpdate(ctx, key);
  return blake2bFinal(ctx);
}

/**
 * Encode a 256-bit key as Bech32 with HRP "zkey".
 */
export function keyToBech32(key: Uint8Array): string {
  const words = bech32.toWords(key);
  return bech32.encode(KEY_HRP, words, 90);
}

/**
 * Decode a Bech32 "zkey" string back to raw bytes.
 */
export function bech32ToKey(encoded: string): Uint8Array {
  const { prefix, words } = bech32.decode(encoded, 90);
  if (prefix !== KEY_HRP) {
    throw new Error(`Expected HRP "${KEY_HRP}", got "${prefix}"`);
  }
  const bytes = bech32.fromWords(words);
  if (bytes.length !== KEY_BYTES) {
    throw new Error(`Expected ${KEY_BYTES} bytes, got ${bytes.length}`);
  }
  return new Uint8Array(bytes);
}
