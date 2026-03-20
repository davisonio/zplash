export interface PaymentParams {
  amount: string;
  desc?: string;
  key: string;
}

/**
 * Build a payment URI using the current origin (works on any domain).
 * Key, amount, and description go in the fragment (#) — never sent to a server.
 */
export function buildPaymentURI(params: PaymentParams): string {
  const base = typeof window !== 'undefined'
    ? `${window.location.origin}/claim`
    : 'https://zplash.app/claim';

  const parts: string[] = [];
  parts.push(`amount=${params.amount}`);
  if (params.desc) {
    parts.push(`desc=${encodeURIComponent(params.desc)}`);
  }
  parts.push(`key=${params.key}`);
  return `${base}#${parts.join('&')}`;
}

/**
 * Parse a payment URI from the full URL.
 */
export function parsePaymentURI(uri: string): PaymentParams {
  const hashIndex = uri.indexOf('#');
  if (hashIndex === -1) {
    throw new Error('No fragment found in URI');
  }

  const fragment = uri.slice(hashIndex + 1);
  const params = new URLSearchParams(fragment);

  const amount = params.get('amount');
  if (!amount) throw new Error('Missing required parameter: amount');

  const key = params.get('key');
  if (!key) throw new Error('Missing required parameter: key');

  const desc = params.get('desc') || undefined;
  return { amount, desc, key };
}

/**
 * Parse the URL fragment directly.
 */
export function parseFragment(hash: string): PaymentParams {
  const fragment = hash.startsWith('#') ? hash.slice(1) : hash;
  if (!fragment) throw new Error('Empty fragment');

  const params = new URLSearchParams(fragment);

  const amount = params.get('amount');
  if (!amount) throw new Error('Missing required parameter: amount');

  const key = params.get('key');
  if (!key) throw new Error('Missing required parameter: key');

  const desc = params.get('desc') || undefined;
  return { amount, desc, key };
}

/**
 * Validate a ZEC amount string.
 */
export function validateAmount(amount: string): string | null {
  if (!amount || amount.trim() === '') return 'Amount is required';
  const num = parseFloat(amount);
  if (isNaN(num) || num <= 0) return 'Amount must be a positive number';
  const parts = amount.split('.');
  if (parts.length > 1 && parts[1].length > 8) return 'Max 8 decimal places';
  return null;
}
