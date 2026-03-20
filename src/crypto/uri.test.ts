import { describe, it, expect } from 'vitest';
import { buildPaymentURI, parsePaymentURI, parseFragment, validateAmount } from './uri';

describe('URI round-trip', () => {
  it('builds and parses with all fields', () => {
    const params = { amount: '0.1', desc: 'Test payment', key: 'zkey1abc123' };
    const uri = buildPaymentURI(params);
    const parsed = parsePaymentURI(uri);
    expect(parsed.amount).toBe('0.1');
    expect(parsed.desc).toBe('Test payment');
    expect(parsed.key).toBe('zkey1abc123');
  });

  it('handles no description', () => {
    const params = { amount: '1.5', key: 'zkey1xyz' };
    const uri = buildPaymentURI(params);
    const parsed = parsePaymentURI(uri);
    expect(parsed.amount).toBe('1.5');
    expect(parsed.desc).toBeUndefined();
  });

  it('percent-encodes special characters', () => {
    const params = { amount: '0.5', desc: 'Hello & welcome!', key: 'zkey1test' };
    const uri = buildPaymentURI(params);
    expect(uri).toContain('Hello%20%26%20welcome!');
    expect(parsePaymentURI(uri).desc).toBe('Hello & welcome!');
  });
});

describe('parsePaymentURI errors', () => {
  it('rejects missing fragment', () => {
    expect(() => parsePaymentURI('https://example.com/claim')).toThrow('fragment');
  });

  it('rejects missing amount', () => {
    expect(() => parsePaymentURI('https://x.com#key=test')).toThrow('amount');
  });

  it('rejects missing key', () => {
    expect(() => parsePaymentURI('https://x.com#amount=1')).toThrow('key');
  });
});

describe('parseFragment', () => {
  it('parses a hash string directly', () => {
    const result = parseFragment('#amount=0.5&key=zkey1abc');
    expect(result.amount).toBe('0.5');
    expect(result.key).toBe('zkey1abc');
  });

  it('works without leading #', () => {
    const result = parseFragment('amount=1&key=zkey1xyz');
    expect(result.amount).toBe('1');
  });

  it('rejects empty input', () => {
    expect(() => parseFragment('')).toThrow();
  });
});

describe('validateAmount', () => {
  it('accepts valid amounts', () => {
    expect(validateAmount('0.1')).toBeNull();
    expect(validateAmount('1')).toBeNull();
    expect(validateAmount('0.00000001')).toBeNull();
  });

  it('rejects empty', () => {
    expect(validateAmount('')).not.toBeNull();
  });

  it('rejects non-positive', () => {
    expect(validateAmount('0')).not.toBeNull();
    expect(validateAmount('-5')).not.toBeNull();
  });

  it('rejects >8 decimals', () => {
    expect(validateAmount('0.000000001')).not.toBeNull();
  });
});
