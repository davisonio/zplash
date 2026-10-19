# Tasks

- [ ] Check by hand: a testnet sweep end to end after the Zcash crates moved from 0.26 to 0.30
  (new `BuildConfig` padding fields and an extra fee-rule argument in `wasm-sweep`).

## Stack

- [ ] Set `VITE_PROXY_URL` for the Worker build. Vercel has it; `pnpm run build` and CI don't yet.
- [ ] Add the domain at the `LAB-DOMAIN` comment in `cloudflare.config.ts` once it's picked.
- [ ] Check the SPA fallback on a preview: `/create`, `/claim`, `/create/`, `/claim/` should return the app.
- [ ] Decide on `api/latest.js`: it needs `@grpc/*`, which isn't a dependency, so it can't load today.
- [ ] After the Worker is live, remove `vercel.json` and `.vercelignore`, and decide on the Vercel link in the README.
