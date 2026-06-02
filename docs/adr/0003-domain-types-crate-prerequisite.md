# ADR 0003 — `mizan-domain-types` Crate as Prerequisite for Track H Extractions

**Status:** Accepted
**Date:** 2026-06-02
**Deciders:** Sami Sayyed (autonomous-loop authorization)
**Track:** H (Code Hygiene & Audit Pass) — prerequisite for PR-H3.b / H3.c / H3.d / H3.e

## Context

PR-H3.a successfully extracted `crates/financial-truth` because the Truth Engine had **no internal dependencies on other mizan-core modules** — its 3 files only used external deps (chrono, rust_decimal, serde, etc.). Consumers depended on it, never the reverse.

PR-H3.b zakat investigation surfaces a different shape. Zakat depends on:

| Import | Defined in | Used for |
|---|---|---|
| `crate::assets::AssetKind` | `mizan-core/src/assets/` | Enum discriminating Cash / Equity / Bond / RealEstate / etc. — drives the Zakatable / Not-Zakatable / Mixed routing |
| `crate::portfolio::holdings::HoldingType` | `mizan-core/src/portfolio/holdings/` | Per-position type tag |
| `crate::portfolio::holdings::HoldingsServiceTrait` | `mizan-core/src/portfolio/holdings/` | Reads holdings for the Zakat calculation |
| `crate::errors::Result` | `mizan-core/src/errors.rs` | Error wrapping |

If `mizan-zakat` were a leaf crate, **all four** would need to either move with it (huge — `AssetKind` and `HoldingsServiceTrait` are shared with insights, synthesis, performance, valuation, every dashboard view) OR live somewhere both `mizan-core` and `mizan-zakat` can see.

The same problem hits:
- **PR-H3.c insights** — depends on holdings, activities, net-worth snapshots
- **PR-H3.d synthesis** — depends on every domain module (it's literally the synthesizer)
- **PR-H3.e csv-import** — depends on activities + accounts + holdings shapes

Without a shared types crate, every extraction would either pull mountains of code with it or create circular workspace deps.

## Decision

Before executing PR-H3.b through H3.e, introduce a **leaf crate `mizan-domain-types`** that owns the cross-cutting domain types every downstream crate (zakat, insights, synthesis, csv-import) needs:

```
mizan-4/crates/domain-types/
├── Cargo.toml          # leaf — only external deps (serde, chrono, rust_decimal)
└── src/
    ├── lib.rs          # DomainTypesError + re-exports
    ├── assets.rs       # AssetKind enum + classification helpers
    ├── holdings.rs     # HoldingType + the *view* struct that read-side consumers need
    ├── activities.rs   # Activity / ActivityKind types (pure data; no service trait)
    ├── currency.rs     # Currency, FxRate (timestamped), no silent fallbacks
    └── period.rs       # Date ranges, lunar-year arithmetic helpers for Hawl
```

**Crucially, `mizan-domain-types` contains DATA types only — no service traits.** Service traits live in the crate that owns the behaviour:

- `HoldingsServiceTrait` stays in `mizan-core/portfolio/holdings` because it's the read-side service
- Downstream crates (zakat, insights, synthesis) define **inputs** as a slice / iterator of `HoldingsView` (a pure data struct in `mizan-domain-types`)
- The desktop calls `holdings_service.snapshot()` → produces `Vec<HoldingsView>` → passes to `mizan_zakat::compute_zakat(&holdings_view, ...)`

This pattern lets each extracted crate be a **pure function over data**, never reaching back into mizan-core for service traits.

### Error type

`DomainTypesError` is intentionally tiny — only validation errors on the data types themselves (`InvalidCurrencyCode(String)`, `InvalidLunarDate(String)`, etc.). Downstream crates wrap or pass through.

## Consequences

**Positive:**
- Every Track H extraction (H3.b/c/d/e) becomes feasible as a leaf crate that depends only on `mizan-domain-types` + its external deps.
- The "each crate owns its types" principle (working-agreement §4.1) compounds — `mizan-zakat` owns Zakat-shaped types; `mizan-domain-types` owns shared shapes; nothing crosses.
- Service-trait/data separation is the textbook Rust pattern (data = inert; behaviour = trait + impl). Producing this separation is positive technical debt repayment regardless of extractions.
- Future Track C predictive layer can also depend on `mizan-domain-types` without pulling `mizan-core`.

**Negative:**
- Adds one more crate to the workspace + one more PR before Track H closes.
- The migration of `AssetKind`, `HoldingType`, etc. has to update every site that imports them across mizan-core. Mechanical but voluminous.
- Some existing types in `mizan-core` are entangled (e.g. `Holding` carries an `Asset` carries a `Quote`). The first cut should move ONLY the types that the extraction targets actually need; the rest can follow per-extraction.

**Follow-ups (tracked):**
- PR-H3.0 (new): extract `mizan-domain-types` crate with `AssetKind` + `HoldingType` + `HoldingsView` + `ActivityView` + `Currency` + lunar-year date helpers + `DomainTypesError`. **MUST land before H3.b.**
- Per-extraction follow-ups (H3.b–e) consume `mizan-domain-types` and define their input types in terms of its `HoldingsView` / `ActivityView` slices.

## Alternatives Considered

**Alternative A: Have `mizan-zakat` etc. depend directly on `mizan-core`.** Rejected — creates the exact circular boundary the Track H extraction is meant to eliminate. Working-agreement §6 95% coverage floor can't be enforced when the crate transitively pulls all of mizan-core's surface.

**Alternative B: Move full `Holding` / `Asset` / `Activity` types into `mizan-domain-types`.** Rejected for the first cut — the existing types carry many fields used only by specific consumers. Moving them whole forces every mizan-core consumer to also depend on the new crate, defeating the leaf goal. Iterative: data types travel as their first consumer extracts; the rest stays in mizan-core until its consumer extracts.

**Alternative C: Inline duplicate types in each extracted crate.** Rejected — duplication breaks the "each crate owns its types" property at the workspace level. A bug in `AssetKind` would need fixing in 5 places.

## Refs

- `docs/plans/08-track-h.md` PR-H3 series
- `docs/plans/track-h-extractions/01-financial-truth.md` — pattern this ADR builds on
- ADR 0002 (financial-truth extraction — the precedent)
- Working-agreement §4.1 (each crate owns its types), §5 (coverage floors), §6 (mutation testing), §17 (no backward-compat shims)
