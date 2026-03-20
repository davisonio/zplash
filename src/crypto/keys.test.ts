import { describe, it, expect } from 'vitest';
import { generateKey, deriveSpendingKey, keyToBech32, bech32ToKey } from './keys';

describe('generateKey', () => {
  it('produces 32 bytes', () => {
    expect(generateKey().length).toBe(32);
  });

  it('produces unique values', () => {
    const a = generateKey();
    const b = generateKey();
    expect(a).not.toEqual(b);
  });
});

describe('bech32 round-trip', () => {
  it('encodes and decodes back to the same key', () => {
    const key = generateKey();
    const encoded = keyToBech32(key);
    const decoded = bech32ToKey(encoded);
    expect(decoded).toEqual(key);
  });

  it('uses the zkey prefix', () => {
    const encoded = keyToBech32(generateKey());
    expect(encoded.startsWith('zkey1')).toBe(true);
  });

  it('rejects wrong prefix', () => {
    const encoded = keyToBech32(generateKey());
    const bad = 'wrong1' + encoded.slice(5);
    expect(() => bech32ToKey(bad)).toThrow();
  });
});

describe('deriveSpendingKey', () => {
  it('produces 32 bytes', () => {
    const sk = deriveSpendingKey(generateKey());
    expect(sk.length).toBe(32);
  });

  it('is deterministic', () => {
    const key = generateKey();
    expect(deriveSpendingKey(key)).toEqual(deriveSpendingKey(key));
  });

  it('different keys give different results', () => {
    const sk1 = deriveSpendingKey(generateKey());
    const sk2 = deriveSpendingKey(generateKey());
    expect(sk1).not.toEqual(sk2);
  });

  it('rejects wrong-length input', () => {
    expect(() => deriveSpendingKey(new Uint8Array(16))).toThrow();
  });
});
