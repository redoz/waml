# Security Policy

## Reporting a vulnerability

Please report security issues **privately** — do **not** open a public GitHub issue, PR, or discussion for a vulnerability.

Email **bi@owox.com** with:

- a description of the issue and its impact,
- steps to reproduce (or a proof of concept),
- the affected component (`packages/okf` or `packages/web`) and version/commit if known.

We aim to acknowledge reports within a few business days and will keep you updated as we investigate and ship a fix. Please give us reasonable time to remediate before any public disclosure. We're happy to credit reporters who'd like acknowledgement.

## Supported versions

This is an actively developed project; security fixes target the latest `main`. There are no long-term support branches.

## Security model (context for reporters)

OKF Canvas is a **purely static, client-side single-page app** — there is no backend, no accounts, and no authentication. The two components are:

- `packages/okf` — a pure parse/serialize library for OKF Markdown bundles (no I/O).
- `packages/web` — the React/Vite SPA that runs entirely in the browser.

Because everything runs in the browser, the relevant properties are:

- **No server, no secrets at rest.** The app ships as static files; it holds no credentials and exchanges none.
- **No network calls for your data.** The app makes no outbound API calls with your model. Your work stays in the page.
- **Local-only storage.** The current model, its name, the view mode, relationship labels and the business goal are kept in the browser's `localStorage`. This is your own data on your own machine; it is not transmitted.
- **URL sharing is client-side.** Sharing packs the model (gzip + JSON) into the URL hash (`#m=…`). That link is generated in the browser and only leaves your machine if you choose to share it — treat a shared link as you would any document, since anyone with it can open the model it encodes.
- **Strict CSP.** The build uses `script-src 'self'` (no inline scripts) to reduce the XSS surface.

We make no broader security guarantees beyond the above. Reports that identify ways client-side data could leak, or that strengthen the CSP/XSS boundary, are especially welcome.
