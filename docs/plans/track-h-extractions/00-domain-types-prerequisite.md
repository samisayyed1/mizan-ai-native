# PR-H3.0 — Extract `crates/domain-types` (Prerequisite for H3.b–e)

**Status:** ⏸️ Planned — must land before PR-H3.b zakat
**Estimated effort:** 1–1.5 days dedicated
**Track:** H (Code Hygiene & Audit Pass) — prerequisite for the remaining 4 extractions
**Decision:** ADR 0003

## Why

H3.b zakat's investigation (see [02-zakat.md](02-zakat.md)) revealed that every remaining Track H extraction (zakat, insights, synthesis, csv-import) shares the same blocker: they all import cross-cutting domain types (`AssetKind`, `HoldingType`, `HoldingsServiceTrait`, `Activity`, etc.) from `mizan-core`. Moving those types with each extraction creates chains of dependencies that defeat the leaf-crate goal.

ADR 0003 resolves this by introducing one leaf crate that owns the **pure data types** the downstream crates read.

## Scope

**In:** A new workspace member `mizan-4/crates/domain-types/` containing **data types only** (no service traits, no I/O, no behaviour):

```
mizan-4/crates/domain-types/
├── Cargo.toml          # leaf — external deps only (serde, chrono, rust_decimal, thiserror)
└── src/
    ├── lib.rs          # DomainTypesError + re-exports
    ├── assets.rs       # AssetKind enum + classification helpers (pure functions)
    ├── holdings.rs     # HoldingType enum + HoldingsView struct (read-only data shape)
    ├── activities.rs   # ActivityKind enum + ActivityView struct
    ├── currency.rs     # Currency code wrapper + FxRate (timestamped, no silent fallback)
    └── period.rs       # DateRange helpers + lunar-year arithmetic for Hawl
```

**Out:** Anything with behaviour. Specifically:
- `HoldingsServiceTrait` stays in `mizan-core/portfolio/holdings/` (it's a service that READS holdings from storage)
- `ActivityServiceTrait` stays in `mizan-core/activities/`
- `FxServiceTrait` stays in `mizan-core/fx/`
- The Zakat / Insights / Synthesis service traits land in their respective extracted crates (H3.b/c/d)

## Design principle

**Data here. Behaviour where it's owned.**

Downstream crates (zakat, insights, synthesis) accept their inputs as a slice / iterator of the View structs from this crate, not as a service-trait dependency. The desktop materialises the views (via existing service traits in mizan-core) and hands the slice to the extracted crate's pure functions.

Example:

```rust
// In mizan-core (consumer)
let holdings: Vec<HoldingsView> = holdings_service.snapshot().await?;
let zakat_report = mizan_zakat::compute(&holdings, inputs)?;
```

vs the current entanglement:

```rust
// In mizan-core
let zakat_report = zakat_service.compute(holdings_service.clone(), inputs).await?;
```

The new shape lets `mizan-zakat` be tested with a hand-built `Vec<HoldingsView>` and never reach back into mizan-core for a service trait.

## What this crate IS

- Inert data structs with `#[derive(Debug, Clone, Serialize, Deserialize)]`
- Enums with explicit discriminants
- Helper free functions on the data (e.g. `period::days_in_lunar_year() -> u16`)
- A single `DomainTypesError` for validation errors on the data itself

## What this crate is NOT

- Not a service layer
- Not an I/O surface
- Not a trait-export crate (other than the data structs implementing standard traits)
- Not a "shared utilities" dumping ground (we have `utils.rs` per-crate for those)

## Step-by-step execution

1. **Create the workspace member** + Cargo.toml with external deps only
2. **Move types**:
   - `mizan-core/src/assets/assets_model.rs::AssetKind` → `domain-types/src/assets.rs::AssetKind`
   - `mizan-core/src/portfolio/holdings/holdings_model.rs::HoldingType` → `domain-types/src/holdings.rs::HoldingType`
   - Define new `HoldingsView` struct in `holdings.rs` with the read-side fields (qty, cost_basis_base, market_value_base, currency, asset_kind, holding_type, sharia_status)
   - Same for `ActivityView` in `activities.rs`
   - Move `Currency` newtype + `FxRate` struct (with timestamp) into `currency.rs`
   - Move lunar-year date helpers into `period.rs`
3. **Workspace Cargo.toml**: add `mizan-domain-types = { path = "crates/domain-types" }`
4. **mizan-core consumer updates**: re-export the moved types from their old locations as backward-compat shims **ONLY for this PR** so the rest of mizan-core can keep compiling. Plan to delete the shims in H3.b–e as each extraction starts using `mizan_domain_types::` directly.
   - **Caveat:** working-agreement §17 bans backward-compat shims as permanent fixtures. The shims here are temporary and explicitly tracked for deletion. Each subsequent PR-H3.b/c/d/e removes the shims for the types it owns.
5. **Verify**: `cargo check --workspace`, `cargo test --workspace --lib`, `cargo clippy -- -D warnings`, `cargo fmt`
6. **Open PR** with self-review checklist + shim-removal tracking

## Self-review checklist

- [ ] Forward-only — no migrations
- [ ] N/A cache-invalidation
- [ ] N/A RLS
- [ ] ADR 0003 captures the architecture
- [ ] Truth Ledger preserved — no truth_engine changes
- [ ] No silent FX — `FxRate` carries timestamp explicitly; `Currency` newtype validates ISO 4217
- [ ] No f64 — Decimal in money fields
- [ ] Shim removal tracked in H3.b–e plans

## What follows

Once PR-H3.0 lands, the remaining extractions execute in order:

1. **PR-H3.b** zakat — depends on `HoldingsView`, `AssetKind`, `HoldingType`, `Currency`
2. **PR-H3.c** insights — depends on `HoldingsView`, `ActivityView`, `DateRange`
3. **PR-H3.d** synthesis — broadest dependency; lands after the others stabilise
4. **PR-H3.e** csv-import — depends on `ActivityView`, `AssetKind`, `Currency`

After H3.e, run PR-H8 (FX disallowed_methods now has named methods to target in `mizan-domain-types::currency::FxRate`) and PR-H9 (full audit → Gate 1).

## Refs

- ADR 0003 — Domain Types Crate as Prerequisite
- `docs/plans/track-h-extractions/01-financial-truth.md` — the proven extraction pattern this builds on
- `docs/plans/track-h-extractions/02-zakat.md` — the first beneficiary of this prereq
- Working-agreement §4.1, §5, §6, §17
