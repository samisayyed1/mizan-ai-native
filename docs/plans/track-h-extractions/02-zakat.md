# PR-H3.b — Extract `crates/zakat`

**Status:** ⏸️ Blocked on PR-H3.0 (`mizan-domain-types` crate) per ADR 0003
**Estimated effort:** 1–2 days post-prerequisite
**Track:** H (Code Hygiene & Audit Pass) — second crate extraction
**Depends on:** PR-H3.0 (`mizan-domain-types`)
**Blocks:** PR-H3.c (insights), PR-H3.d (synthesis), PR-H3.e (csv-import) — all need the same prereq

## Why this isn't a clone of PR-H3.a

PR-H3.a (truth_engine) extracted cleanly because the module had zero internal mizan-core dependencies — only external deps (chrono, rust_decimal, serde, sha2).

H3.b zakat is different. The 4 files (532 lines total) import:

```
use crate::assets::AssetKind;
use crate::errors::Result;
use crate::portfolio::holdings::{HoldingType, HoldingsServiceTrait};
```

If `mizan-zakat` were a leaf crate, `AssetKind` + `HoldingType` + `HoldingsServiceTrait` would have to move with it. But those types are shared across insights, synthesis, performance, valuation — every dashboard view in the app. Moving them with zakat creates a dependency chain that defeats the leaf-crate goal.

[ADR 0003](../../adr/0003-domain-types-crate-prerequisite.md) proposes the resolution: a new leaf crate `mizan-domain-types` carrying the cross-cutting domain types (data only — no service traits) that every Track H extraction can depend on.

## Scope (after PR-H3.0 lands)

**In:** Move `mizan-core/src/portfolio/zakat/` (`mod.rs` + `zakat_model.rs` + `zakat_service.rs` + `zakat_traits.rs`, 532 lines) into `mizan-4/crates/zakat/src/`.

**Out:**
- The Maliki + Hanbali school additions (Track F PR-F2–F5) — those are scholarly-sign-off gated (Gate 4 reassigned to Uncle Ferox per current goal). H3.b only moves the existing Hanafi + Shafi'i code.
- The Hawl tracking module (Track F PR-F11 has its own migration; the engine that READS hawl_anchors can be a follow-up).
- The Pay-Zakat Stripe flow (Track F PR-F14).

## Crate shape

```
mizan-4/crates/zakat/
├── Cargo.toml                # leaf; deps: mizan-domain-types + external
└── src/
    ├── lib.rs                # ZakatError + Result + re-exports + ZakatService trait
    ├── model.rs              # ZakatInputs + ZakatReport (moved from zakat_model.rs)
    ├── traits.rs             # ZakatServiceTrait (moved from zakat_traits.rs)
    └── service.rs            # the engine (moved from zakat_service.rs)
```

### Error type (matching the financial-truth pattern)

```rust
#[derive(Debug, thiserror::Error)]
pub enum ZakatError {
    #[error("invalid Zakat input: {0}")]
    InvalidInput(String),

    #[error("missing FX rate for {pair} at {timestamp}")]
    MissingFxRate { pair: String, timestamp: String },

    #[error("Nisab data unavailable: {0}")]
    NisabUnavailable(String),

    #[error("unknown scholarly school: {0}")]
    UnknownSchool(String),
}

pub type Result<T> = std::result::Result<T, ZakatError>;
```

### Consumer migration scope

```
mizan-4/apps/tauri/src/context/registry.rs    — type refs
mizan-4/apps/tauri/src/context/providers.rs   — service construction
mizan-4/apps/tauri/src/commands/zakat.rs      — command handler
```

Only 3 files (much smaller surface than PR-H3.a's 7-file migration).

### test-utils feature (same pattern as financial-truth)

Add `mizan-zakat = { features = ["test-utils"] }` for any consumer whose tests construct fake zakat reports. Initial scope: no consumers need this (the actual zakat service is deterministic given inputs); skip the feature unless a real need surfaces.

## Step-by-step (after PR-H3.0)

1. Create `mizan-4/crates/zakat/` with the shape above
2. Move 4 files, rename per the shape (`zakat_model.rs` → `model.rs`, etc.)
3. Rewrite imports:
   - `crate::assets::AssetKind` → `mizan_domain_types::AssetKind`
   - `crate::portfolio::holdings::{HoldingType, HoldingsServiceTrait}` →
     - `HoldingType` → `mizan_domain_types::HoldingType`
     - `HoldingsServiceTrait` → **redefine as a pure input slice** — `compute_zakat(holdings: &[HoldingsView], inputs: ZakatInputs) -> Result<ZakatReport>`. The desktop caller materialises the slice via the existing `HoldingsServiceTrait` in mizan-core.
   - `crate::errors::Result` → `crate::Result` (the local ZakatError-based alias)
4. Update Cargo.toml in workspace + the 3 consumer crates
5. Delete `mizan-core/src/portfolio/zakat/` + remove `pub mod zakat;` from `mizan-core/src/portfolio/mod.rs`
6. Update the 3 consumer files
7. Verify: `cargo check --workspace`, `cargo test --workspace --lib`, `cargo clippy ... -- -D warnings`, `cargo fmt`
8. Open PR with self-review checklist (two-reviewer rule via the working-agreement §5 list; self-approve per autonomous-execution authorization)

## Self-review checklist (for the actual PR)

- [ ] Forward-only — no migrations affected
- [ ] N/A cache-invalidation
- [ ] N/A RLS
- [ ] ADR rationale captured (this plan + ADR 0003 + a new ADR 0004 for zakat extraction)
- [ ] Truth Ledger preserved — Zakat calculations write to the Truth Ledger per spec §11.5; that path lives in `mizan-core`'s activities_service which already uses `mizan_financial_truth` — unaffected by this PR
- [ ] No silent FX — `ZakatError::MissingFxRate` surfaces explicitly (working-agreement §0 rule 2)
- [ ] No f64 — `rust_decimal::Decimal` throughout

## Open questions (none — all defaults per autonomous-loop authorization)

- ZakatError variants chosen to match the patterns the existing engine raises (validation, missing FX, missing nisab, unknown school). If new variants surface during extraction, they get added with `#[error("...")]` per the existing pattern.
- HoldingsView shape lands in PR-H3.0 — should carry exactly what zakat reads: `qty: Decimal`, `cost_basis_base: Decimal`, `market_value_base: Decimal`, `currency: String`, `asset_kind: AssetKind`, `holding_type: HoldingType`, optional `sharia_status: Option<ShariaStatus>` (for compliant filtering).

## Refs

- ADR 0003 — `mizan-domain-types` prerequisite
- ADR 0011 — Holdings Metadata Design (the `sharia_status` field zakat will eventually read)
- Spec §11 (Zakat Engine)
- `docs/plans/06-track-f.md` Track F plan (Maliki + Hanbali school additions happen there, not here)
