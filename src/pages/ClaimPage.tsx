import { useState, useEffect } from 'react';
import { parseFragment } from '../crypto/uri';
import { bech32ToKey, deriveSpendingKey } from '../crypto/keys';
import { deriveSaplingAddress } from '../crypto/address';
import { sweepFunds } from '../crypto/sweep';
import { CopyButton } from '../components/CopyButton';

interface ClaimData {
  amount: string;
  desc?: string;
  key: string;
  address: string;
}

function formatZec(zatoshi: number) {
  return (zatoshi / 1e8).toFixed(8).replace(/\.?0+$/, '');
}

export function ClaimPage() {
  const [claim, setClaim] = useState<ClaimData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [recipientAddr, setRecipientAddr] = useState('');
  const [status, setStatus] = useState<'idle' | 'sweeping' | 'done' | 'error'>('idle');
  const [progress, setProgress] = useState('');
  const [result, setResult] = useState<{
    txid: string;
    amountZatoshi: number;
    feeZatoshi: number;
  } | null>(null);
  const [sweepError, setSweepError] = useState('');

  useEffect(() => {
    async function load() {
      try {
        const hash = window.location.hash;
        if (!hash || hash === '#') {
          setError('No payment data in this link.');
          return;
        }
        const params = parseFragment(hash);
        const rawKey = bech32ToKey(params.key);
        const sk = deriveSpendingKey(rawKey);
        const address = await deriveSaplingAddress(sk, false);
        setClaim({ amount: params.amount, desc: params.desc, key: params.key, address });
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Invalid payment link');
      }
    }
    load();
  }, []);

  const handleSweep = async () => {
    if (!claim || !recipientAddr.trim()) return;
    setStatus('sweeping');
    setSweepError('');
    setProgress('Starting...');

    try {
      const res = await sweepFunds(claim.key, recipientAddr.trim(), setProgress);
      setResult({
        txid: res.txid,
        amountZatoshi: res.amountZatoshi,
        feeZatoshi: res.feeZatoshi,
      });
      setStatus('done');
    } catch (err) {
      setSweepError(err instanceof Error ? err.message : 'Sweep failed');
      setStatus('error');
    }
  };

  if (error) {
    return (
      <div className="page claim-error">
        <h2>Bad link</h2>
        <p>{error}</p>
        <p className="hint">Check you copied the full URL including everything after the #.</p>
      </div>
    );
  }

  if (!claim) {
    return <div className="page"><p className="text-dim">Loading...</p></div>;
  }

  return (
    <div className="page claim">
      <div className="claim-amount">
        <span className="amount-value">{claim.amount}</span>
        <span className="amount-unit">ZEC</span>
      </div>

      {claim.desc && <p className="claim-desc">{claim.desc}</p>}

      <div className="warning">
        This link is the money. Don't share it unless you want someone else to have it.
      </div>

      {status === 'done' && result ? (
        <div className="sweep-success">
          <h3>Claimed</h3>
          <p>{formatZec(result.amountZatoshi)} ZEC is on its way to your wallet.</p>
          <p className="hint">Shielded claim fee: {formatZec(result.feeZatoshi)} ZEC.</p>
          <div className="copyable">
            <code className="copyable-text">{result.txid}</code>
            <CopyButton text={result.txid} label="txid" />
          </div>
        </div>
      ) : (
        <div className="claim-actions">
          <div className="sweep-form">
            <h3>Claim to your shielded wallet</h3>
            <p className="hint">
              Paste a shielded Zcash address (`u1...` or `zs1...`). We scan about the last 24 hours
              of blocks, then build the claim transaction in your browser. The key never leaves.
            </p>
            <div className="field">
              <input
                type="text"
                placeholder="u1... or zs1..."
                value={recipientAddr}
                onChange={(e) => setRecipientAddr(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleSweep()}
                disabled={status === 'sweeping'}
              />
            </div>
            <p className="hint">You'll receive the funded amount minus a small shielded claim fee.</p>
            {status === 'sweeping' && <p className="progress-msg">{progress}</p>}
            {sweepError && <div className="error-msg">{sweepError}</div>}
            <button
              className="btn btn-primary btn-large"
              onClick={handleSweep}
              disabled={status === 'sweeping' || !recipientAddr.trim()}
            >
              {status === 'sweeping' ? 'Working...' : 'Claim funds'}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
