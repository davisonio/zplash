# Zplash

**Live demo: [zplash.vercel.app](https://zplash.vercel.app/)**

A working implementation of [ZIP-324](https://zips.z.cash/zip-0324) for Zcash. Fund a shielded address, then let someone claim the funds with a link.

## How it works

1. Create a payment link: the app generates an ephemeral key and derives a Sapling payment address
2. Fund the address: send ZEC using Zodl or any Zcash wallet
3. Share the link: the recipient opens it, pastes a shielded address, and claims the funds in the browser

The secret key lives in the URL fragment (`#`), which browsers never send to servers. The proxy only serves block data, proving params, and transaction broadcast.

## Quick start

```bash
pnpm install
cd wasm-sweep && wasm-pack build --target web --out-dir pkg && cd ..
cd proxy && cargo run && cd ..
pnpm dev
```

On macOS, Apple's clang can't build the `secp256k1-sys` C code for wasm32. Install LLVM (`brew install llvm`) and run `wasm-pack` with `CC_wasm32_unknown_unknown=$(brew --prefix llvm)/bin/clang AR_wasm32_unknown_unknown=$(brew --prefix llvm)/bin/llvm-ar`.

Open [http://localhost:5173](http://localhost:5173). The frontend expects the proxy at `http://localhost:3001` unless `VITE_PROXY_URL` is set.

## URI format

```
https://zplash.vercel.app/claim#amount=0.1&desc=Lunch&key=zkey1...
```

| Parameter | Required | Description |
|-----------|----------|-------------|
| `amount`  | Yes      | Decimal ZEC (max 8 decimal places) |
| `desc`    | No       | Percent-encoded description |
| `key`     | Yes      | 256-bit key, Bech32-encoded with HRP "zkey" |

## Crypto

- Key generation: 256-bit CSPRNG via `crypto.getRandomValues`
- Key derivation: `BLAKE2b-256(key, personal="Zcash_PaymentURI")`
- Key encoding: Bech32 with HRP `zkey`
- Address derivation: browser calls Rust/WASM, which derives a real Sapling address from `ExtendedSpendingKey::master()`
- Claim flow: browser scans compact blocks, trial-decrypts Sapling outputs, builds the spend, proves it, and broadcasts it through the proxy

## Deploy

The frontend is static assets served by a Cloudflare Worker (`cloudflare.config.ts`, with the `@cloudflare/vite-plugin` in `vite.config.ts`). `pnpm run build` writes the Worker output to `.cloudflare/output/`. Build, then deploy:

```bash
pnpm run build
pnpm run deploy
```

Set `VITE_PROXY_URL` to the public URL of the proxy when building for deploy. It defaults to `http://localhost:3001`. The proxy is a separate Fly app (`proxy/fly.toml`) and deploys on its own. No secrets are needed.

## Tests

```bash
pnpm test
```

Covers Bech32 round-trip encoding, BLAKE2b key derivation, URI construction/parsing, and amount validation. Rust pieces can be checked with `cargo check` in `proxy/` and `wasm-sweep/`.

## Architecture

```
src/
  crypto/
    keys.ts       Key generation, BLAKE2b derivation, Bech32 encode/decode
    uri.ts        ZIP-324 URI construction and parsing
    address.ts    Sapling address derivation via WASM
    sweep.ts      Client-side scan, claim-state check, proving param download, sweep
  pages/
    Landing.tsx   Home page with explanation
    CreateLink.tsx  Payment link creation flow
    ClaimPage.tsx   Claim/receive flow
  components/
    QRCode.tsx    QR code display wrapper
    CopyButton.tsx  Click-to-copy with feedback
proxy/
  src/main.rs     REST to lightwalletd proxy, plus proving param passthrough
wasm-sweep/
  src/lib.rs      Sapling note scanning, witness tracking, and transaction building
```

## Current limits

- The claim flow currently scans about the last 24 hours of blocks. Older links may not be found.
- The recipient needs a shielded Zcash address to receive the funds. `u1...` and `zs1...` work.
- The claimer receives the funded amount minus a small shielded transaction fee.

## Privacy

- All funded addresses are shielded Sapling addresses
- Keys stay in the URL fragment and are used only in the browser
- The proxy only receives block range requests, tree state requests, proving param requests, and signed transaction bytes
- No analytics, tracking, cookies, or localStorage
