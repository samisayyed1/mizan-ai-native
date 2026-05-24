---
name: mizan-clean-rebuild
description: User-invoked only. Kills running Mizan, wipes local app data, rebuilds desktop, reinstalls, and launches — the loop used while validating fresh-install behaviour.
disable-model-invocation: true
---

# Clean rebuild + reinstall loop (manual only)

For when "does this actually work from a clean state?" is the question.
Destructive — wipes local Mizan data. Always confirm with the user.

## Steps

```bash
# 1. Kill the running app
osascript -e 'quit app "Mizan"' 2>/dev/null || true
pkill -f Mizan || true

# 2. Wipe local data (encrypted SQLite + cached settings)
rm -rf ~/Library/Application\ Support/app.mizan
rm -rf ~/Library/Caches/app.mizan
rm -rf ~/Library/Preferences/app.mizan.plist

# 3. Rebuild
cd ~/Documents/mizan-ai-native/mizan-4
pnpm --filter frontend build
pnpm tauri build --target aarch64-apple-darwin

# 4. Reinstall
DMG=$(ls src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/Mizan_*.dmg | head -n 1)
hdiutil attach "$DMG" -nobrowse -quiet
MNT=$(ls -d /Volumes/Mizan_* | head -n 1)
rm -rf /Applications/Mizan.app
cp -R "$MNT"/Mizan.app /Applications/
hdiutil detach "$MNT" -quiet

# 5. Launch
open /Applications/Mizan.app
```

## After launch — walk these manually

Per v3 §14 Definition of Done:

1. Onboarding 3-step completes.
2. 3 example liabilities appear with real-looking numbers.
3. Sign in to Mizan Connect; `/v1/me` returns Free tier.
4. Ticker conveyor populates within 5 s.
5. AI assistant blocked with upgrade gate (no managed AI on Free).
6. Upgrade to Silver via test-mode Stripe; entitlements refresh
   within 5 s.
7. AI assistant unlocks; conversational create_account works.
8. Upgrade to Gold; Plaid + SnapTrade unlock.
9. Connect Plaid sandbox + SnapTrade sandbox; provider data flows.
10. Health Center shows live status; break a provider, see specific
    error; reconnect succeeds.

If any step fails, **don't paper over it.** Surface the failure mode
clearly and decide whether it's a Phase-N gate violation.
