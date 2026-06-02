//! Performance budgets — locked thresholds for hot-path operations.
//!
//! Per MIZAN_AI_NATIVE_PLAN.md v3.1 §A19: every release locks specific
//! ops to a latency budget so regressions surface in CI before users
//! feel them. This module is *just* the assertion harness — concrete
//! ops live in their own modules; the budget tests below import them.
//!
//! Budgets are intentionally generous (5-10× expected actual cost) to
//! avoid red flakes from runner-load noise. If a real-world operation
//! comes in 100× slower than the budget, that's a regression worth
//! investigating; if it's 2× slower than the previous run, that's
//! within normal jitter.
//!
//! Set `MIZAN_PERF_BUDGET_STRICT=1` to fail at expected-latency × 1.5
//! instead of the generous default. Useful when manually re-tuning.

use std::time::{Duration, Instant};

/// Run `f` and assert it completes inside `budget`. Returns the actual
/// elapsed duration so the caller can also assert on more specific
/// bounds (e.g. "under 50ms" + "and over 0ns to prove it actually ran").
///
/// Panics on budget violation rather than returning Err — perf budgets
/// are test-time invariants and the message is meant to surface in the
/// test runner.
pub fn within_budget<F: FnOnce() -> R, R>(label: &str, budget: Duration, f: F) -> (R, Duration) {
    let start = Instant::now();
    let result = f();
    let elapsed = start.elapsed();

    let effective_budget = if std::env::var("MIZAN_PERF_BUDGET_STRICT").ok().as_deref() == Some("1")
    {
        // Strict mode tightens to the expected latency. Operators use
        // this when investigating "this got slower" — the failure
        // surfaces at the first real regression instead of waiting
        // for the 10× headroom to be eaten.
        budget / 5
    } else {
        budget
    };

    assert!(
        elapsed < effective_budget,
        "perf budget exceeded: {label} took {elapsed:?}, budget was {effective_budget:?}",
    );
    (result, elapsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn within_budget_passes_under_threshold() {
        let (_, elapsed) = within_budget("noop", Duration::from_millis(100), || {});
        assert!(elapsed < Duration::from_millis(100));
    }

    #[test]
    #[should_panic(expected = "perf budget exceeded")]
    fn within_budget_panics_over_threshold() {
        within_budget("slow", Duration::from_nanos(1), || {
            std::thread::sleep(Duration::from_millis(2));
        });
    }

    // ─── Real hot-path budgets ────────────────────────────────────────
    //
    // These lock in current production-grade perf. If they fail in CI,
    // someone introduced a regression and the PR should explain why.

    #[test]
    fn budget_account_options_build() {
        use crate::accounts::Account;
        use chrono::NaiveDateTime;

        // Building a 1000-account options vector — what every
        // record_activity / create_account AI tool does on every call.
        let accounts: Vec<Account> = (0..1000)
            .map(|i| Account {
                id: format!("acc-{}", i),
                name: format!("Account {}", i),
                account_type: "CASH".to_string(),
                group: None,
                currency: "USD".to_string(),
                is_default: false,
                is_active: true,
                created_at: NaiveDateTime::default(),
                updated_at: NaiveDateTime::default(),
                platform_id: None,
                account_number: None,
                meta: None,
                provider: Some("MANUAL".to_string()),
                provider_account_id: None,
                is_archived: false,
                tracking_mode: Default::default(),
            })
            .collect();

        let (count, _) = within_budget(
            "account_options_build (1000 accounts)",
            Duration::from_millis(10),
            || {
                accounts
                    .iter()
                    .filter(|a| a.is_active && !a.is_archived)
                    .count()
            },
        );
        assert_eq!(count, 1000);
    }

    #[test]
    fn budget_decimal_addition_loop() {
        use rust_decimal::Decimal;

        // Summing 10k Decimal values — what net-worth aggregations and
        // zakat assessments lean on. SQLite returns Decimals as i64+scale
        // pairs and we sum them in user space.
        let values: Vec<Decimal> = (0..10_000)
            .map(|i| Decimal::from(i) / Decimal::from(100))
            .collect();

        let (total, _) = within_budget(
            "decimal_sum (10k values)",
            Duration::from_millis(50),
            || values.iter().sum::<Decimal>(),
        );
        assert!(total > Decimal::ZERO);
    }
}
