---
name: mizan-tier-gate
description: Use when gating a feature behind Free / Silver / Gold. Provides the three-layer pattern (frontend useCapability, backend gated() IPC helper, route guard) and the upgrade-modal UX template.
---

# Tier-gate a feature

Mizan ships three tiers: **Free / Silver / Gold**. Capabilities flow
from the cloud's `/v1/me` response into the desktop's entitlements
cache. Gating happens at three layers — all three are required, no
exceptions.

## Layer 1: frontend hook

```tsx
// apps/frontend/src/domain/account/capabilities.ts
export const CAPABILITY_TIERS: Record<Capability, Tier> = {
  // ...
  newCapability: "Silver",
};

// in component
const { entitled, requiredTier } = useCapability("newCapability");
if (!entitled) return <UpgradeGate requiredTier={requiredTier} feature="…" />;
```

The component must render the locked variant when `!entitled`, never
silently disable. The locked variant shows: what the feature does,
which tier unlocks it, a "Upgrade" CTA opening the Stripe Checkout URL
in the system browser.

## Layer 2: backend IPC gate

```rust
// apps/tauri/src/commands/<feature>.rs
use crate::commands::entitlements::gated;

#[tauri::command]
pub async fn new_capability_action(
    state: tauri::State<'_, AppState>,
    args: Args,
) -> Result<Output, String> {
    let user = state.current_user().await?;
    gated(&user, Capability::NewCapability)?;
    // ...
}
```

The `gated()` helper returns `Err("entitlement_required: …")` so the
frontend can show the upgrade modal. Backend gate is the security
boundary; the frontend gate is UX only.

## Layer 3: route guard

```tsx
// apps/frontend/src/routes.tsx
{
  path: "/new-feature",
  element: (
    <CapabilityGuard capability="newCapability">
      <NewFeaturePage />
    </CapabilityGuard>
  ),
}
```

`CapabilityGuard` shows the locked landing page if `!entitled` (NOT a
500 or blank screen).

## Cloud-side mapping

Update `mizan-connect/src/entitlements.rs` to add the capability to the
correct tier in the `tier_capabilities` table. Update `/v1/me` response
DTO to include the new flag.

## Tier matrix sanity check

After the change, verify:

| Tier   | Sees the feature?    |
| ------ | -------------------- |
| Free   | Locked + upgrade CTA |
| Silver | Locked / unlocked    |
| Gold   | Unlocked             |

The locked CTA copy must be specific: "Upgrade to Silver to use CSV
import — your local data stays exactly where it is." Never use generic
"upgrade for more" copy.

## When done

- `mizan-pr-checklist` skill.
- Manual smoke: cycle through Free/Silver/Gold by changing
  `subscriptions.tier` directly in test DB, confirm the gate behaves
  in each.
