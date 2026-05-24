---
name: mizan-release
description: User-invoked only. Bundles pnpm tauri build + DMG → artifacts/ + git commit/push. Disabled for model invocation; run via /skill mizan-release.
disable-model-invocation: true
---

# Release the desktop DMG (manual only)

Do not invoke this automatically. The user runs it when an actual
release is wanted — never as part of normal phase work.

## Pre-flight

1. Confirm `MIZAN_ALLOW_PRODUCTION=1` is set (the release inevitably
   needs to push artifacts; the PreToolUse hook will block otherwise).
2. Run the `mizan-pr-checklist` skill first.
3. Run the v3.1 §25 "No Silent Failure" certification if this is a
   real production release.

## Build steps

```bash
cd ~/Documents/mizan-ai-native/mizan-4

pnpm --filter frontend build
pnpm tauri build --target aarch64-apple-darwin
```

The DMG lands at
`mizan-4/src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/*.dmg`.
Copy to `artifacts/` with a versioned name:

```bash
cp src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/Mizan_*.dmg \
   ../artifacts/Mizan-v$(cat src-tauri/tauri.conf.json | jq -r .version)-aarch64.dmg
```

## Commit + push (user-confirmed)

Show the user the artifact size, the SHA-256, and the diff vs. previous
DMG. Wait for explicit OK before pushing.

```bash
git add ../artifacts/
git commit -m "release: desktop v$VERSION (aarch64)"
git push origin main
```

## Never

- Never auto-run this in a turn that started with another task.
- Never push without an explicit ack from Sami in the same turn.
- Never bundle live Plaid / SnapTrade / Stripe credentials into the DMG —
  the build must use only sandbox / test secrets until production gate
  passes.
