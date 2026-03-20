import { useState, useCallback } from 'react';
import { generateKey, deriveSpendingKey, keyToBech32 } from '../crypto/keys';
import { buildPaymentURI, validateAmount } from '../crypto/uri';
import { deriveSaplingAddress } from '../crypto/address';
import { QRCode } from '../components/QRCode';
import { CopyButton } from '../components/CopyButton';

interface GeneratedPayment {
  uri: string;
  address: string;
  amount: string;
  desc?: string;
}

export function CreateLink() {
  const [amount, setAmount] = useState('');
  const [desc, setDesc] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [generating, setGenerating] = useState(false);
  const [payment, setPayment] = useState<GeneratedPayment | null>(null);

  const handleGenerate = useCallback(async () => {
    const amountError = validateAmount(amount);
    if (amountError) {
      setError(amountError);
      return;
    }
    setError(null);
    setGenerating(true);

    try {
      const key = generateKey();
      const sk = deriveSpendingKey(key);
      const bech32Key = keyToBech32(key);
      const address = await deriveSaplingAddress(sk, false);
      const uri = buildPaymentURI({
        amount: amount.trim(),
        desc: desc.trim() || undefined,
        key: bech32Key,
      });

      setPayment({
        uri,
        address,
        amount: amount.trim(),
        desc: desc.trim() || undefined,
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to generate address');
    } finally {
      setGenerating(false);
    }
  }, [amount, desc]);

  if (payment) {
    return (
      <div className="page create-result">
        <h2>Done. Two steps left.</h2>

        <div className="result-section">
          <h3>Step 1: Fund it</h3>
          <p className="hint">
            Send exactly <strong>{payment.amount} ZEC</strong> to this address in Zodl or any Zcash wallet.
          </p>
          <p className="hint">
            The recipient will get the funded amount minus a small shielded claim fee.
          </p>
          <div className="copyable">
            <code className="copyable-text">{payment.address}</code>
            <CopyButton text={payment.address} />
          </div>
          <QRCode
            value={`zcash:${payment.address}?amount=${payment.amount}`}
            label="scan to send"
          />
        </div>

        <div className="result-section">
          <h3>Step 2: Share the link</h3>
          <p className="hint">
            Once funded, send this link to whoever you want to receive the ZEC. Ask them to claim it within 24 hours.
          </p>
          <div className="copyable">
            <code className="copyable-text">{payment.uri}</code>
            <CopyButton text={payment.uri} />
          </div>
          <QRCode value={payment.uri} label="scan to claim" />
        </div>

        <div className="warning">
          This link contains the key to the funds. Anyone with it can claim them.
        </div>

        <button className="btn btn-secondary btn-large" onClick={() => setPayment(null)}>
          Create another
        </button>
      </div>
    );
  }

  return (
    <div className="page create-form">
      <h2>Create a payment link</h2>
      <p className="subtitle">Pick an amount and we'll generate a shielded address you can fund, plus a claim link you can share.</p>

      <div className="form">
        <div className="field">
          <label htmlFor="amount">Amount (ZEC)</label>
          <input
            id="amount"
            type="text"
            inputMode="decimal"
            placeholder="0.01"
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleGenerate()}
            autoFocus
          />
        </div>

        <div className="field">
          <label htmlFor="desc">Note (optional)</label>
          <input
            id="desc"
            type="text"
            placeholder="Lunch money"
            value={desc}
            onChange={(e) => setDesc(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleGenerate()}
            maxLength={200}
          />
        </div>

        {error && <div className="error-msg">{error}</div>}

        <p className="hint">Current limit: links are scanned for about 24 hours after funding.</p>

        <button
          className="btn btn-primary btn-large"
          onClick={handleGenerate}
          disabled={generating}
        >
          {generating ? 'Generating...' : 'Generate link'}
        </button>
      </div>
    </div>
  );
}
