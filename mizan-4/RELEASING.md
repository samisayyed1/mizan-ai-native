# Releasing Mizan desktop

How to ship a new desktop release to end users via GitHub Releases.

The pipeline is fully automated: tag a version, push the tag, GitHub Actions
builds installers for four targets and publishes them as a public Release.

## TL;DR

```bash
# Bump version (see versioning rules below for choosing X.Y.Z)
./scripts/bump-version.sh 1.0.0   # if a script exists, otherwise edit by hand
git commit -am "chore: release v1.0.0"
git tag v1.0.0
git push origin main --tags
```

Then watch <https://github.com/samisayyed1/mizan-4/actions> for the `Release`
workflow. ~25 minutes later, installers are live at
<https://github.com/samisayyed1/mizan-4/releases/latest>.

## Versioning

We use [SemVer](https://semver.org) with one Mizan-specific rule about the
leading number.

### Choosing the next version

| Change                                                                  | Bump                      |
| ----------------------------------------------------------------------- | ------------------------- |
| Bug fixes only, no behavior change                                      | **patch** (1.0.0 → 1.0.1) |
| New features, backward-compatible                                       | **minor** (1.0.0 → 1.1.0) |
| Breaking change to the on-disk DB schema, settings format, or addon API | **major** (1.0.0 → 2.0.0) |

A breaking change to the **Mizan Connect cloud API** does not require a desktop
major bump if the desktop app degrades cleanly to offline mode.

### Why we start at 3.x, not 1.0.0

The repo's lineage starts at the Wealthfolio fork (which was on 1.x at the time
of fork). The current `[workspace.package].version` is **3.3.0**, inherited from
upstream and reset against Mizan's own roadmap. Future releases continue from
there.

If we ever need a clean reset to `1.0.0`, do it as a deliberate ADR — don't do
it casually because it forces every downstream check that parses the version
(Tauri bundle versioning, telemetry, etc.) to re-baseline.

### Pre-releases

Append `-rc.N`, `-beta.N`, `-alpha.N` to the version:

```
v1.0.0-rc.1
v1.0.0-beta.3
```

The release workflow handles pre-release bundles by skipping the Windows MSI
(WiX requires numeric-only versions) and falling back to the NSIS bundler. Mark
such tags as pre-releases on the GitHub UI if you want them hidden from
`releases/latest`.

## Files to update before tagging

The `version` lives in two places. Both must match the tag, minus the leading
`v`.

- [`Cargo.toml`](Cargo.toml) — `[workspace.package].version` (drives every Rust
  crate including `apps/tauri`)
- [`apps/tauri/tauri.conf.json`](apps/tauri/tauri.conf.json) — top-level
  `version` (drives the bundle metadata + tauri-action's `__VERSION__`
  substitution)

Optional but encouraged:

- `CHANGELOG.md` — append a section for the new tag with notable changes. The
  release body is currently a static template; switch to populating from
  `CHANGELOG.md` in `release.yml` if you want.

## What gets built

`.github/workflows/release.yml` runs four matrix jobs in parallel and attaches
their bundles to a single GitHub Release named after the tag:

| Target              | Runner           | Bundle filename pattern      |
| ------------------- | ---------------- | ---------------------------- |
| macOS Apple Silicon | `macos-14`       | `Mizan_<ver>_aarch64.dmg`    |
| macOS Intel         | `macos-13`       | `Mizan_<ver>_x64.dmg`        |
| Windows x64         | `windows-latest` | `Mizan_<ver>_x64_en-US.msi`  |
| Linux x64           | `ubuntu-22.04`   | `Mizan_<ver>_amd64.AppImage` |

The release is published immediately (`releaseDraft: false`). If you want to
review before going live, change `releaseDraft: true` in the workflow before
tagging.

In parallel, `.github/workflows/release-server.yml` builds a self-hoster tarball
(`mizan-server-<ver>-linux-amd64.tar.gz`) and attaches it to the same release.
This is for users running Mizan in web mode on their own server, not for desktop
end users.

## Predictable download URLs (for the landing page)

GitHub Releases provides a stable `releases/latest/download/<asset>` redirect
that tracks whichever release is marked latest. The landing page can hardcode
against:

```
https://github.com/samisayyed1/mizan-4/releases/latest/download/Mizan_3.3.0_aarch64.dmg
https://github.com/samisayyed1/mizan-4/releases/latest/download/Mizan_3.3.0_x64.dmg
https://github.com/samisayyed1/mizan-4/releases/latest/download/Mizan_3.3.0_x64_en-US.msi
https://github.com/samisayyed1/mizan-4/releases/latest/download/Mizan_3.3.0_amd64.AppImage
```

⚠️ These URLs include the version number, so the landing page must update them
on every release. If you want truly version-stable URLs, add a small redirect
step in the release workflow that re-uploads each asset under a version-less
name (e.g. `Mizan_aarch64.dmg`). Not worth the complexity until v1 stabilizes.

## After the release

1. **Verify download** — actually click each link from a fresh browser tab and
   confirm the asset downloads.
2. **Smoke test one installer** — install on a real machine (your Mac is fine).
   Open the app. Confirm it boots, the dashboard renders, no console errors in
   the Tauri webview.
3. **Update landing page** if the version number changed in URLs.
4. **Announce** — if it's a milestone release.

## Code signing — currently SKIPPED

Bundles are unsigned by deliberate Chunk DL-1 decision. End users see one-time
warnings:

- **macOS Gatekeeper:** "Mizan can't be opened because Apple cannot check it for
  malicious software." Workaround: right-click → Open.
- **Windows SmartScreen:** "Windows protected your PC." Workaround: More info →
  Run anyway.
- **Linux:** no warning (AppImage doesn't use OS-level code signing).

To enable signing later:

- **macOS:** $99/year Apple Developer Program. Set the secrets
  `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`,
  `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` on the repo. tauri-action will
  pick them up automatically — no workflow changes needed.
- **Windows:** $200-ish/year Authenticode cert (DigiCert, Sectigo). Set
  `WINDOWS_CERTIFICATE` + `WINDOWS_CERTIFICATE_PASSWORD` secrets. Workflow needs
  a `WIX_TOOLSET_DIR` step + cert import.

Both deferred to a separate chunk if/when there's demand to escape the warnings.

## Auto-update — currently DISABLED

Tauri's `updater` plugin is in the dependency tree but not wired up. End users
manually download new versions when we publish them. To enable:

1. Generate a Tauri signing key (`tauri signer generate`).
2. Set `TAURI_SIGNING_PRIVATE_KEY` + `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
   secrets.
3. Configure `plugins.updater` in `tauri.conf.json` with an endpoint (e.g. a
   static `latest.json` on GitHub Pages or in the release).
4. Wire the updater check into the React app on startup.

This is a separate chunk. Out of scope for DL-1.

## Rolling back a release

GitHub Releases doesn't support "rollback" per se — you can only delete a
release or mark it as not-latest.

- **Delete the bad release** from
  <https://github.com/samisayyed1/mizan-4/releases>. The git tag stays unless
  you also delete that.
- **Delete the tag locally and on origin:**
  ```bash
  git tag -d v1.0.0
  git push origin :refs/tags/v1.0.0
  ```
- **Re-tag fixed code under a new patch version** (don't reuse the same tag —
  caches and CDN edges may have served the old asset).

## Troubleshooting

| Symptom                                     | Likely cause                                                                                                                                                                          |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Workflow fires but no Release created       | `permissions: contents: write` missing or the runner couldn't reach GitHub's API. Check the workflow logs.                                                                            |
| One platform's job fails, others succeed    | `fail-fast: false` is on, so the Release exists with 3/4 assets. Re-run just the failing job from the Actions UI; tauri-action will append the missing asset to the existing Release. |
| Windows MSI step fails on a pre-release tag | WiX rejects non-numeric version pre-release suffixes. The release.yml falls back to NSIS for tagged pre-releases — confirm the `--bundles nsis` arg is being passed.                  |
| Linux AppImage missing icons / odd theme    | Missing libappindicator3 / librsvg in the runner. Re-check the `apt-get install` step in release.yml.                                                                                 |
| `tauri-action: not found`                   | Runner doesn't have the Tauri Rust target installed. The workflow uses `dtolnay/rust-toolchain@stable` with the matrix-specified target — verify the matrix entry's `target` field.   |
