import { Link } from 'react-router-dom';

export function Landing() {
  return (
    <div className="page landing">
      <div className="hero">
        <div className="hero-glow" />
        <h1 className="brand"><span className="z">Z</span>plash</h1>
        <p className="tagline">
          Send ZEC with a link. Fund a shielded address, then let the recipient claim it into any shielded wallet.
        </p>
        <p className="hint">Links are scanned for about 24 hours after funding.</p>
        <Link to="/create" className="btn btn-primary btn-large">
          Create a payment link
        </Link>
      </div>

      <div className="how-it-works">
        <div className="how-step">
          <div className="num">01</div>
          <h3>Create</h3>
          <p>Pick an amount. We generate a unique shielded address.</p>
        </div>
        <div className="how-step">
          <div className="num">02</div>
          <h3>Fund</h3>
          <p>Send ZEC to the address from Zodl or any wallet.</p>
        </div>
        <div className="how-step">
          <div className="num">03</div>
          <h3>Share</h3>
          <p>Send the link. The recipient pastes a shielded address and claims in the browser.</p>
        </div>
      </div>

      <div className="info-footer">
        <p>
          Built on{' '}
          <a href="https://zips.z.cash/zip-0324" target="_blank" rel="noopener noreferrer">
            ZIP-324
          </a>
          {' '}/ Keys stay in the browser / Claim transaction built client-side / Shielded by default
        </p>
      </div>
    </div>
  );
}
