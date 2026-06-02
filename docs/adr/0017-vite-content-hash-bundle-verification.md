# ADR 0017 — Vite content-hash bundle verification on WebView load

| Status | ✅ Accepted (autonomous-execution authority — Track I close-out work) |
|---|---|
| Date | 2026-06-03 |
| Author | ai (auditor; under autonomous-execution authorization) |
| Related | [ADR 0009 — Updater snapshot & rollback design](0009-updater-snapshot-and-rollback-design.md), Working-agreement §19.7 (cache eviction + bundle integrity), Track I PR-I8.a/b |

## Context

Mizan Desktop ships its frontend as a static bundle produced by Vite. Vite emits asset filenames that include a content hash (e.g. `assets/index-A1B2C3.js`) so that a content change yields a new filename — this is the standard cache-busting mechanism.

In production, Tauri 2 embeds the built frontend into the binary via `tauri-build`. The frontend is served to the WebView through Tauri's asset resolver (not via HTTP). The WebView's own cache layer (WKWebView on macOS, WebView2 on Windows, WebKitGTK on Linux) caches the served bytes.

Two failure modes this ADR addresses:

1. **Stale WebView cache after binary upgrade.** A user on v3.4.0 had `assets/index-OLD.js` cached. They upgrade to v3.4.1 whose binary contains `assets/index-NEW.js`. If the index.html in the new binary still points at the new filename (it does — Vite emits a fresh index.html), the cache miss resolves correctly. BUT if for any reason the WebView's HTML cache served the old index.html while the JS bundle came from the new binary, the user sees a runtime error (`__vite_legacy_polyfill is not defined` and similar).
2. **Tampered bundle / supply-chain interference.** A malicious extension or post-install script could swap one of the asset files in the on-disk Tauri bundle. Without verification, the WebView would happily execute the modified JS with the user's full keychain access.

## Decision

Ship a manifest-based content-hash verifier that runs at desktop startup, BEFORE the WebView is allowed to paint.

The implementation lands in TWO PRs:

### PR-I8.a (this PR — ship scaffold + ADR)

- Define the verifier's public API: `verify_embedded_bundle(...) -> Result<BundleVerification, BundleError>` in `apps/tauri/src/webview_assets.rs`.
- The verifier reads a manifest (JSON: filename → blake3 content hash), iterates each entry, computes blake3 of the served bytes, and compares.
- Provide a fully-tested `verify_against_manifest` helper that takes the manifest + a `FetchAsset` trait so unit tests can run against tempfile fixtures without the Tauri runtime.
- Wire a NO-OP call site in `apps/tauri/src/lib.rs::setup` that logs INFO ("PR-I8.a scaffold — verifier wired; full embedding lands in I8.b") so the integration site is reserved.
- DO NOT yet require the manifest to exist — `verify_embedded_bundle` returns `Ok(Skipped { reason })` when the manifest is absent.

### PR-I8.b (next — wire the production embedding)

- Add a `tauri-build` post-build hook (or a `build.rs` step) that:
  1. Runs `pnpm build` to produce `mizan-4/apps/frontend/dist/`
  2. Walks `dist/` and writes `dist/manifest.json` with `{ "filename": "<rel-path>", "blake3": "<hex>" }` per asset
  3. Embeds `dist/manifest.json` into the Tauri bundle via `include_str!`
- Change `verify_embedded_bundle` from `Ok(Skipped)` to `Err(BundleError::ManifestMissing)` when the embedded manifest is absent.
- On mismatch: log `error!`, call `WebviewWindow::clear_all_browsing_data()` (Tauri 2 API; falls back to platform-specific cache wipe on older WebView runtimes), trigger a reload via `WebviewWindow::eval("window.location.reload()")`.

## Rationale

**Why blake3 over SHA-256?**
- blake3 is already a workspace dep (Truth Ledger chain hashing — see `crates/financial-truth/src/service.rs`)
- ~5–10× faster on the asset sizes we care about (tens-of-KB JS bundles)
- Cryptographically identical security guarantees for our purposes (collision resistance)

**Why a separate manifest rather than computing hashes from index.html?**
- Vite's `manifest.json` is already produced when `build.manifest = true`. Re-using it avoids reinventing the parser.
- The manifest is the canonical "what the build produced" record. Computing hashes from `index.html` would only catch HTML-referenced assets, missing dynamically-loaded chunks.

**Why verify ALL assets, not just `index.html`?**
- A targeted swap of `assets/vendor-X.js` would bypass an index-only check while still executing arbitrary JS in the keychain context.

**Why wipe cache + reload rather than refuse to launch?**
- The most common cause is a stale WebView cache, NOT tampering. Refuse-to-launch would create a worse UX than the cache-wipe path for the dominant case.
- For genuine tampering, the wipe + reload still triggers a re-fetch from the on-disk binary, where the manifest comparison runs AGAIN. Persistent mismatch → log + structured Sentry event for investigation. PR-I8.c adds the "after N consecutive failures, refuse to launch" backstop.

**Why scaffold + ADR before the embedding?**
- The verifier's API + tests + integration point are reviewable on their own. The build-system change (tauri-build hook) is a separate concern that benefits from focused review.
- Working-agreement §A21 explicitly endorses staged PRs over megastacks: "Each PR < 500 lines; each PR independently reviewable."

## Consequences

**Positive:**
- Stale WebView caches are wiped + reloaded automatically — no more "please reinstall" support tickets after upgrades
- A tampered bundle is detected before any user code executes inside the WebView (key authority gate)
- The verifier composes with PR-I5's self-test pattern (same pre-WebView-paint slot, same logging conventions)

**Negative:**
- Startup adds a manifest read + N×blake3 computations. For a typical Mizan frontend (~30 assets totalling ~2 MB), this is ~10–30ms on a modern laptop — well within the §A19 cold-start budget. Measured in PR-I8.b once embedding is live.
- The build pipeline grows a new step (manifest generation). Tested in CI before merge.

**Risks:**
- The cache-wipe path is platform-specific. We document each platform's behavior in PR-I8.b's commit body + verify on each in beta channel before stable.
- Catastrophic failure mode: manifest matches but the bundle is genuinely broken (e.g. a JS runtime exception). Out of scope for this verifier — that's what the self-test's `truth_ledger_chain_head` + `crypto_round_trip` catch.

## Alternatives considered

- **Use Subresource Integrity (SRI) `<script integrity="...">`** — works for HTTP-served assets but Tauri uses an internal asset resolver, not HTTP fetch. Adapting SRI to Tauri's resolver is more work than the manifest approach.
- **Sign the bundle and verify signature only at install time** — covers tampering but not WebView cache staleness. The manifest + content-hash approach covers both with one mechanism.
- **Trust the Tauri bundle implicitly** — what we do today. The post-upgrade stale-cache issue is a known support-burden tax. Verification eliminates it.

## Implementation (PR-I8.a)

The scaffold ships with:
- `apps/tauri/src/webview_assets.rs` — `verify_against_manifest`, `Manifest`, `BundleError`, `FetchAsset` trait, 6 unit tests against tempfile.
- `apps/tauri/src/lib.rs::setup` — no-op call site that logs the scaffold marker.
- This ADR.

PR-I8.b adds the production embedding + replaces the no-op with the real call.
