# PR-H3.a — Extract `crates/financial-truth`

**Status:** ⏸️ Planned — investigation done 2026-06-02, execution deferred to dedicated session
**Estimated effort:** 1–2 days (multi-cycle PR with iterative CI fixes)
**Track:** H (Code Hygiene & Audit Pass) — first of 5 crate extractions
**Blocks:** PR-H3.b (zakat), PR-H8 (FX disallowed_methods lint targets financial-truth API)

## Why an extraction plan instead of the extraction

The autonomous session that scoped this PR documented:

- **Volume**: 638 lines in `truth_engine/` (3 files) PLUS scattered FIFO/TWR/IRR logic across `portfolio/{synthesis,snapshot,performance,valuation,holdings,net_worth}.rs` and `activities/`. The "complete" financial-truth extraction touches ~15 files in mizan-core + ~3 in mizan-storage-sqlite + tests.
- **Dependency entanglement**: `truth_engine/service.rs` uses `crate::Result` which wraps `mizan_core::Error` — an enum with 10+ variants for activities/fx/quotes/etc. The extraction can't simply move files; it must either (a) introduce a smaller `FinancialTruthError` + `Result` in the new crate or (b) move all dependent error variants too (which defeats the extraction).
- **Consumer update fan-out**: 15+ `crate::truth_engine::` references in `activities/activities_service.rs` alone + tests + storage-sqlite + ai crate. Each must update its imports and possibly its Cargo.toml.

A clean PR requires:

1. Design the `FinancialTruthError` enum + `Result` alias
2. Create the new crate workspace member
3. Move `truth_engine/{mod,model,service}.rs` (lift into crate root since it's the only thing in the crate at PR-H3.a — full module structure can land in PR-H3.a.2 when FIFO/TWR/IRR migrate)
4. Update consumers: `activities_service.rs`, `activities_service_tests.rs`, `storage-sqlite/truth_ledger/repository.rs`, plus any AI crate consumers
5. Add `mizan-financial-truth` as a dependency to each consumer's Cargo.toml
6. Delete the original `mizan-core/src/truth_engine/` directory (no backward-compat shim per working agreement §17)
7. Verify `cargo test --workspace` + `cargo clippy --workspace --all-targets -- -D warnings` green
8. Run the §A19 perf budget check (truth ledger appends are on the hot path)
9. Verify the QA-P2.4 golden test (Truth Ledger chain integrity) still passes
10. Add the 95% coverage floor enforcement (see §H3.a-coverage below)

## Scope IN (this extraction)

Just `truth_engine/`. The 3 files. Plus all consumer updates.

## Scope OUT (deferred to PR-H3.a.2)

- `portfolio/synthesis.rs` FIFO + cost-basis logic
- `portfolio/snapshot/{snapshot_service,positions_model,holdings_calculator}.rs` (TWR/positions math)
- `portfolio/performance/{performance_service,flow_classifier,performance_model}.rs` (TWR/IRR)
- `portfolio/valuation/valuation_calculator.rs`
- `portfolio/split_adjustment.rs`
- `portfolio/net_worth/net_worth_service.rs` (Net Worth = Assets - Liabilities calc)

These move in PR-H3.a.2 once the new crate scaffolding is proven by PR-H3.a.

## Step-by-step execution plan

### Step 1 — Create the crate

```
mizan-4/crates/financial-truth/
├── Cargo.toml
├── src/
│   ├── lib.rs           (defines FinancialTruthError + Result + re-exports)
│   ├── ledger.rs        (moved from truth_engine/mod.rs, renamed)
│   ├── model.rs         (moved from truth_engine/model.rs verbatim)
│   └── service.rs       (moved from truth_engine/service.rs, with `crate::Result` → `crate::Result` shadowing the new local Result)
```

### Step 2 — `lib.rs` shape

```rust
//! Mizan Financial Truth — immutable hash-chained ledger + (future) FIFO/TWR/IRR
//!
//! Per working-agreement §0 rule 1 (Truth Ledger sanctity), this crate is the
//! ONLY home for code that emits hash-chained financial-state events. 95%
//! coverage floor + two-reviewer rule per working-agreement §5.

pub mod ledger;
pub mod model;
pub mod service;

pub use ledger::*;
pub use model::*;
pub use service::*;

#[derive(Debug, thiserror::Error)]
pub enum FinancialTruthError {
    #[error("ledger integrity violation: {0}")]
    LedgerIntegrity(#[from] LedgerIntegrityError),

    #[error("serialization failure: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("retry queue is at capacity")]
    RetryQueueFull,
}

pub type Result<T> = std::result::Result<T, FinancialTruthError>;
```

### Step 3 — Cargo.toml

```toml
[package]
name = "mizan-financial-truth"
version.workspace = true
edition.workspace = true
description = "Hash-chained immutable financial ledger + FIFO/TWR/IRR (Track H PR-H3.a)"
license = "AGPL-3.0"

[dependencies]
async-trait = { workspace = true }
chrono = { workspace = true }
rust_decimal = { workspace = true }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
sha2 = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true, features = ["sync"] }

[lints]
workspace = true
```

### Step 4 — Consumer updates

For each file in this list, replace `crate::truth_engine::` with `mizan_financial_truth::` and add `mizan-financial-truth = { workspace = true }` to the consumer's Cargo.toml:

- `mizan-4/crates/core/src/activities/activities_service.rs` (~15 references)
- `mizan-4/crates/core/src/activities/activities_service_tests.rs`
- `mizan-4/crates/storage-sqlite/src/truth_ledger/repository.rs`
- Any other matches from `grep -rln 'truth_engine' mizan-4/crates/`

### Step 5 — Delete originals

```bash
rm -rf mizan-4/crates/core/src/truth_engine/
```

Remove the `pub mod truth_engine;` line from `mizan-4/crates/core/src/lib.rs`.

### Step 6 — Workspace dependency

Add to root `mizan-4/Cargo.toml`:

```toml
[workspace.dependencies]
# ... existing ...
mizan-financial-truth = { path = "crates/financial-truth" }
```

### Step 7 — Verify

```bash
cd mizan-4
cargo check --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --lib
cargo fmt --all -- --check
```

The QA-P2.4 Truth Ledger chain-integrity golden test moves with the model and should pass identically.

### Step 8 — 95% Coverage enforcement

After extraction, add to the nightly-mutants workflow:

```yaml
- name: Mutants — financial-truth
  run: |
    cargo mutants --no-shuffle --jobs 2 \
      --package mizan-financial-truth \
      --minimum-test-timeout 180 \
      --error-rate 5
```

Same for `cargo tarpaulin` line-coverage in the hygiene job.

## Self-review checklist (for the actual PR-H3.a)

- [ ] Forward-only — no migrations affected
- [ ] N/A cache-invalidation
- [ ] N/A RLS
- [ ] ADR rationale captured in `docs/adr/0002-extract-financial-truth-crate.md` (planned)
- [ ] Truth Ledger preserved — the move is a literal lift; chain integrity test passes
- [ ] No silent FX — no FX code in this crate
- [ ] No f64 — Decimal throughout
- [ ] `cargo test --workspace` green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` green
- [ ] 95% line + branch coverage on the new crate (verified locally with `cargo tarpaulin -p mizan-financial-truth`)

## Open questions

- **`FinancialTruthError::Serde(#[from] serde_json::Error)`** — should the new error type wrap serde_json errors or push them up as boxed dyn Error? Recommend wrap, since canonical-payload serialisation is structural to the ledger.
- **`InMemoryTruthLedger`** — the in-memory impl exists for tests. Move it WITH the crate (it's tested code, and downstream crates' tests need it). Recommend yes.

## Next sessions

Once PR-H3.a lands, the remaining extractions follow the same pattern, in order:

- PR-H3.b: `crates/zakat` (Hanafi + Shafi'i + Maliki/Hanbali after Gate 4)
- PR-H3.c: `crates/insights` (deterministic rules + insight engine)
- PR-H3.d: `crates/synthesis` (dashboard headline figures, allocation breakdowns)
- PR-H3.e: `crates/csv-import`

Then PR-H5 (qa-passes scaffold), PR-H8 (FX disallowed_methods targeting now-existing financial-truth API), then PR-H9 audit execution → **GATE 1**.
