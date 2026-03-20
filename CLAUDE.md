# CLAUDE.md — Project Instructions for Claude Code

## Project: Zplash (ZIP-324 Implementation)

You are building a hackathon MVP of ZIP-324 (URI-Encapsulated Payments) for Zcash. This lets users send ZEC via a URL — the Venmo/Zelle UX but for Zcash.

## Core User Flow

1. Sender creates a payment link on the web app
2. App generates an ephemeral key, derives a Sapling payment address
3. Sender funds that address via Zodl or any Zcash wallet
4. Sender shares the link (e.g. `https://claim.zcash.co/claim#amount=0.1&key=...`)
5. Recipient clicks the link, sees amount/description, claims the funds

## Tech Stack

- React 19 + TypeScript + Vite
- `bech32` for key encoding
- `blakejs` for BLAKE2b-256 key derivation
- `qrcode.react` for QR codes
- React Router for /create and /claim routes
- No backend — everything runs client-side. The key lives in the URL fragment (#) which is never sent to any server.

## ZIP-324 URI Format

```
https://claim.zcash.co/claim#amount={zec_amount}&desc={percent_encoded}&key={bech32_key}
```

- `key`: 256-bit CSPRNG value, Bech32-encoded with HRP "zkey"
- `amount`: decimal ZEC, period separator, max 8 decimal places
- `desc`: optional, percent-encoded UTF-8
- Everything after # is the fragment — never leaves the client

## Key Derivation

```
key = crypto.getRandomValues(new Uint8Array(32))
sk = BLAKE2b-256(key, personalization="Zcash_PaymentURI")
```

## Critical Rules

1. **Shielded only.** Never generate or display transparent addresses.
2. **Client-side only.** The spending key must never leave the browser.
3. **No AI slop.** Every word must read like a human wrote it with care.
4. **Ship something that works.** Prioritize the happy path over edge cases.
5. **Testnet first.** Default to testnet (TAZ) for safety.

## Don't

- Don't use localStorage for keys (XSS risk)
- Don't make network requests that include the key
- Don't generate transparent addresses
- Don't over-engineer
- Don't add analytics or tracking
