import { defineConfig } from "cf/config";

export default defineConfig({
  worker: {
    name: "zplash",
    compatibilityDate: "2026-10-01",
    // LAB-DOMAIN: domains: ["zplash.<lab domain>"] once it's picked
    workersDev: true,
    previewUrls: false,
    observability: { enabled: true, issues: { enabled: true } },
    // Client-side routes (/create, /claim) fall back to index.html, as the old vercel.json rewrite did.
    assets: { notFoundHandling: "single-page-application" },
  },
});
