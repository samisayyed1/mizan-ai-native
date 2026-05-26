//! Agent recipes — named, opinionated bundles of (system prompt
//! + tool subset + verification spec) for common multi-step goals.
//!
//! A recipe is what makes an "agent" feel curated rather than
//! generic. The user says "set up my portfolio from this CSV" and
//! the chat dispatcher picks the [`PortfolioFromCsv`] recipe, which:
//!   1. Restricts the LLM to exactly the tools that goal needs
//!      (create_account, parse_csv, record_activities, query_holdings).
//!   2. Adds domain-specific guidance to the system prompt
//!      ("derive the account name from the CSV filename, default
//!      currency to USD unless the filename hints otherwise").
//!   3. Specifies what the agent must verify before reporting done
//!      ("sum of activity costs in the new account must equal the
//!      sum of cost-basis cells in the CSV within $0.01").
//!
//! Recipes are detected by routing the user's message + attachments
//! through [`detect_recipe`]. Free-form goals that don't match any
//! recipe fall back to the generic chat dispatcher.

use crate::agent_planner::ToolDescriptor;

/// One recipe. The chat dispatcher reads `.system_prompt_addendum`,
/// `.tools`, and `.example_plans` when building the planner prompt.
#[derive(Debug, Clone)]
pub struct AgentRecipe {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    /// Prose appended to the generic planner system prompt.
    pub system_prompt_addendum: &'static str,
    /// Names of tools the agent is allowed to call for this recipe.
    /// The chat dispatcher filters the global registry to this list.
    pub tools: &'static [&'static str],
    /// Few-shot example plans. The planner prompt injects these so the
    /// LLM has concrete templates to pattern-match against.
    pub example_plans: &'static [&'static str],
    /// Detection signals — message keywords that hint this recipe.
    pub detection_keywords: &'static [&'static str],
    /// Whether this recipe requires a file attachment to be picked.
    pub requires_attachment: bool,
    /// Whether this recipe is currently Gold-tier-only. Free / Silver
    /// users see "Upgrade to enable" instead.
    pub gold_only: bool,
}

/// All built-in recipes. Add new ones here as they're authored.
pub const ALL_RECIPES: &[AgentRecipe] = &[
    PORTFOLIO_FROM_CSV,
    GOAL_PLANNER,
    BROKER_RECONCILER,
    ALLOCATION_REBALANCER,
    ZAKAT_CALCULATOR,
];

// ─── PortfolioFromCsv ──────────────────────────────────────────────────
//
// The flow that just frustrated the user:
//   "make a new portfolio and add these stocks" + CSV attached.
//
// Today: routes to import_csv → opens a 402-row mapping wizard with no
//   target account selected → user has to scroll and pick.
//
// Agentic: parses the CSV, creates the account first, populates it,
//   verifies totals, reports.

pub const PORTFOLIO_FROM_CSV: AgentRecipe = AgentRecipe {
    id: "portfolio_from_csv",
    label: "Set up portfolio from CSV",
    description: "Create a new portfolio/account and populate it with the rows in an attached CSV. Verifies totals before reporting done.",
    system_prompt_addendum: r#"
RECIPE: PortfolioFromCsv

The user has attached a CSV of broker activities and asked to create
a new portfolio populated with those rows. Your plan MUST follow this
shape:

  1. `create_account` — name derived from the CSV filename (strip
     extension, ".csv", "(1)" etc.) or from explicit user mention
     ("US Stocks"). Currency = USD unless the filename or rows
     suggest otherwise (EUR / GBP / CAD).

  2. `parse_csv` — feed the LLM-detected column mappings into the
     deterministic parser. Pass `accountId` from step 1's output.

  3. `record_activities` — bulk write the parsed rows. The runtime
     batches behind the scenes; you emit ONE step.

  4. `query_account_summary` — read back to confirm.

  5. `verify_totals` (verify step) — assert the activity cost sum
     in the new account equals the cost-basis sum in the source CSV
     within $0.01 tolerance.

Do NOT call `import_csv` (the mapping-wizard tool) — that's the
legacy path, this recipe BYPASSES it.

Do NOT ask the user to confirm anything between steps. The runtime
provides one batch Undo at the end if they want to reverse the run.
"#,
    tools: &[
        "create_account",
        "parse_csv",
        "record_activities",
        "query_account_summary",
        "verify_totals",
    ],
    example_plans: &[
        r#"[
  {"id":"a","tool":"create_account","args":{"name":"US Stocks","currency":"USD","accountType":"SECURITIES"},"dependsOn":[],"summary":"Create the US Stocks portfolio","verify":null},
  {"id":"b","tool":"parse_csv","args":{"accountId":"<a.accountId>","filename":"portfolio US Stocks (1).csv","columnMappings":{"date":"Trade Date","symbol":"Symbol","quantity":"Quantity","price":"Price","type":"Side"}},"dependsOn":["a"],"summary":"Parse the 402 rows in the CSV","verify":null},
  {"id":"c","tool":"record_activities","args":{"accountId":"<a.accountId>","rows":"<b.rows>"},"dependsOn":["b"],"summary":"Record 402 buy/sell activities","verify":null},
  {"id":"d","tool":"query_account_summary","args":{"accountId":"<a.accountId>"},"dependsOn":["c"],"summary":"Read back the new account state","verify":null},
  {"id":"e","tool":"verify_totals","args":{"accountId":"<a.accountId>","expectedTotalFromCsv":"<b.totalCost>"},"dependsOn":["d"],"summary":"Verify activity totals equal CSV cost basis","verify":{"kind":"equal_within","actualTool":"query_account_summary","actualPath":"$.totalCost","expected":"<b.totalCost>","tolerance":0.01}}
]"#,
    ],
    detection_keywords: &[
        "new portfolio",
        "create portfolio",
        "make portfolio",
        "set up portfolio",
        "create account",
        "add these stocks",
        "import these",
        "populate",
        "fresh portfolio",
    ],
    requires_attachment: true,
    gold_only: true,
};

// ─── GoalPlanner ───────────────────────────────────────────────────────
//
// "I want to retire at 55 with $2M" / "How much do I need to save
// monthly to have $500k by 2035?"
//
// The agent reads current portfolio state, computes the savings rate
// or return assumption required, and creates the corresponding Goal
// record with a glidepath.

pub const GOAL_PLANNER: AgentRecipe = AgentRecipe {
    id: "goal_planner",
    label: "Plan a financial goal",
    description: "Decompose a fuzzy goal ('retire at 55 with $2M') into a savings plan + Goal record with glidepath.",
    system_prompt_addendum: r#"
RECIPE: GoalPlanner

The user has stated a fuzzy financial goal. Your plan:

  1. `query_portfolio_summary` — get current net worth, asset
     allocation, monthly savings rate from existing activities.

  2. `compute_savings_required` — given target amount + target date
     + current state + reasonable return assumption (7% real for
     diversified equity, 4% for cash, blended for mixed), output the
     monthly contribution needed.

  3. `create_goal` — instantiate the goal with the computed target,
     date, monthly contribution, and a glidepath.

  4. `verify_goal_consistency` (verify step) — assert the created
     goal's projected final value equals the user's stated target
     within 1%.

If the goal target is unreachable with realistic assumptions, emit
a single step `abort_with_message` that tells the user what's
infeasible and what a realistic alternative would be.
"#,
    tools: &[
        "query_portfolio_summary",
        "compute_savings_required",
        "create_goal",
        "verify_goal_consistency",
        "abort_with_message",
    ],
    example_plans: &[],
    detection_keywords: &[
        "retire at",
        "save for",
        "save by",
        "goal of",
        "i want to have",
        "reach $",
        "by 20",
        "retirement",
    ],
    requires_attachment: false,
    gold_only: true,
};

// ─── BrokerReconciler ──────────────────────────────────────────────────

pub const BROKER_RECONCILER: AgentRecipe = AgentRecipe {
    id: "broker_reconciler",
    label: "Reconcile broker account",
    description: "Diff broker positions vs our activities; find the discrepancy; draft the correcting transaction(s).",
    system_prompt_addendum: r#"
RECIPE: BrokerReconciler

The user said something like "my Schwab account is off by $300" or
"Plaid says I own 105 shares of AAPL but Mizan says 100". Your plan:

  1. `query_account_summary` — get our state for the named account.
  2. `fetch_broker_positions` — pull live broker positions via Plaid.
  3. `diff_positions` — produce a structured list of mismatches.
  4. For each mismatch, `draft_correcting_activity` (you may emit
     multiple parallel steps with no `dependsOn` between them).
  5. `record_activities` — apply the drafts.
  6. `verify_reconciliation` — re-diff and assert empty.
"#,
    tools: &[
        "query_account_summary",
        "fetch_broker_positions",
        "diff_positions",
        "draft_correcting_activity",
        "record_activities",
        "verify_reconciliation",
    ],
    example_plans: &[],
    detection_keywords: &[
        "off by",
        "reconcile",
        "discrepancy",
        "doesn't match",
        "mismatch",
        "broker says",
        "plaid says",
    ],
    requires_attachment: false,
    gold_only: true,
};

// ─── AllocationRebalancer ──────────────────────────────────────────────

pub const ALLOCATION_REBALANCER: AgentRecipe = AgentRecipe {
    id: "allocation_rebalancer",
    label: "Rebalance allocation",
    description: "Compute the trades needed to get back to a target asset allocation.",
    system_prompt_addendum: r#"
RECIPE: AllocationRebalancer

The user said "rebalance to 60/40" or "I'm too heavy in tech, fix it":

  1. `query_current_allocation` — actual percentages.
  2. `compute_rebalance_trades` — given target + current, produce
     buy/sell rows that move the portfolio to the target within a
     tax-aware band (don't sell at a loss less than 30d after a buy,
     prefer rebalancing via new contributions when possible).
  3. `record_activities` — apply the trades as DRAFTS (rebalancing
     touches money, so this is the one recipe where step output
     waits for explicit user confirmation in the progress card).
  4. `verify_allocation_within_band` — post-trade allocation matches
     target within +/- 1%.
"#,
    tools: &[
        "query_current_allocation",
        "compute_rebalance_trades",
        "record_activities",
        "verify_allocation_within_band",
    ],
    example_plans: &[],
    detection_keywords: &["rebalance", "too heavy in", "60/40", "asset allocation", "drift"],
    requires_attachment: false,
    gold_only: true,
};

// ─── ZakatCalculator ───────────────────────────────────────────────────

pub const ZAKAT_CALCULATOR: AgentRecipe = AgentRecipe {
    id: "zakat_calculator",
    label: "Calculate zakat owed",
    description: "Net-zakatable-wealth calculation across cash, gold, silver, qualifying investments, minus debts. Reports the 2.5% owed.",
    system_prompt_addendum: r#"
RECIPE: ZakatCalculator

Islamic zakat is owed on liquid wealth held for one lunar year above
the nisab threshold. Your plan:

  1. `query_cash_balances` — all cash accounts in base currency.
  2. `query_metal_holdings` — gold + silver positions, converted to
     current spot prices.
  3. `query_qualifying_investments` — securities held for >1 lunar
     year (Hijri calendar lookups in the tool).
  4. `query_liabilities` — debts due in the next year.
  5. `compute_zakat_due` — net (cash + metals + investments) - debts,
     verify > nisab, compute 2.5%.
  6. `verify_zakat_calculation` (verify step) — natural-language
     assertion that all six items reconcile.
"#,
    tools: &[
        "query_cash_balances",
        "query_metal_holdings",
        "query_qualifying_investments",
        "query_liabilities",
        "compute_zakat_due",
        "verify_zakat_calculation",
    ],
    example_plans: &[],
    detection_keywords: &["zakat", "زكاة", "alms", "khums"],
    requires_attachment: false,
    gold_only: true,
};

// ─── Detection ─────────────────────────────────────────────────────────

/// Pick the recipe (if any) that best matches the user's message +
/// attachments. Returns `None` if the message doesn't match a recipe
/// — the chat dispatcher falls back to the generic single-turn chat
/// in that case.
pub fn detect_recipe<'a>(
    user_message: &str,
    has_attachment: bool,
) -> Option<&'a AgentRecipe> {
    let lower = user_message.to_lowercase();
    let mut best: Option<(&AgentRecipe, usize)> = None;
    for recipe in ALL_RECIPES {
        if recipe.requires_attachment && !has_attachment {
            continue;
        }
        let hits = recipe
            .detection_keywords
            .iter()
            .filter(|kw| lower.contains(*kw))
            .count();
        if hits > 0 {
            match best {
                None => best = Some((recipe, hits)),
                Some((_, current_hits)) if hits > current_hits => {
                    best = Some((recipe, hits));
                }
                _ => {}
            }
        }
    }
    best.map(|(r, _)| r)
}

/// Filter a tool catalog to the subset a given recipe allows. The
/// chat dispatcher passes the filtered list into [`PromptBasedPlanner`]
/// so the LLM can't invent calls to tools outside the recipe's scope.
pub fn filter_tools_for_recipe(
    recipe: &AgentRecipe,
    all_tools: &[ToolDescriptor],
) -> Vec<ToolDescriptor> {
    all_tools
        .iter()
        .filter(|t| recipe.tools.iter().any(|name| *name == t.name))
        .cloned()
        .collect()
}

/// Compose the planner system-prompt addendum from the recipe +
/// any few-shot examples. The agent_planner prepends the generic
/// schema/constraints; this fills in the recipe-specific bits.
pub fn build_recipe_addendum(recipe: &AgentRecipe) -> String {
    let mut s = String::new();
    s.push_str(recipe.system_prompt_addendum);
    if !recipe.example_plans.is_empty() {
        s.push_str("\n\n### Example plans for this recipe\n\n");
        for ex in recipe.example_plans {
            s.push_str("```json\n");
            s.push_str(ex.trim());
            s.push_str("\n```\n\n");
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn detects_portfolio_from_csv_when_keyword_and_attachment_present() {
        let r = detect_recipe("make a new portfolio and add these stocks", true);
        assert!(r.is_some());
        assert_eq!(r.unwrap().id, "portfolio_from_csv");
    }

    #[test]
    fn does_not_detect_portfolio_from_csv_without_attachment() {
        let r = detect_recipe("make a new portfolio", false);
        // With no attachment the portfolio_from_csv recipe is skipped
        // because it requires a CSV. No other recipe matches just
        // "make a new portfolio".
        assert!(r.is_none());
    }

    #[test]
    fn detects_goal_planner() {
        let r = detect_recipe("I want to retire at 55 with $2M", false);
        assert!(r.is_some());
        assert_eq!(r.unwrap().id, "goal_planner");
    }

    #[test]
    fn detects_broker_reconciler() {
        let r = detect_recipe("Schwab is off by $300", false);
        assert!(r.is_some());
        assert_eq!(r.unwrap().id, "broker_reconciler");
    }

    #[test]
    fn detects_zakat() {
        let r = detect_recipe("calculate my zakat", false);
        assert!(r.is_some());
        assert_eq!(r.unwrap().id, "zakat_calculator");
    }

    #[test]
    fn returns_none_for_unmatched_goal() {
        let r = detect_recipe("what's the weather", false);
        assert!(r.is_none());
    }

    #[test]
    fn filter_tools_returns_only_allowed_subset() {
        let all = vec![
            ToolDescriptor {
                name: "create_account".into(),
                one_line_summary: "x".into(),
                args_schema: serde_json::json!({}),
                mutates: true,
            },
            ToolDescriptor {
                name: "delete_world".into(),
                one_line_summary: "x".into(),
                args_schema: serde_json::json!({}),
                mutates: true,
            },
        ];
        let filtered = filter_tools_for_recipe(PORTFOLIO_FROM_CSV_REF, &all);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "create_account");
    }

    /// Borrow alias so tests can pass a reference even though the
    /// constants are `'static`. (Just keeps the test ergonomic.)
    const PORTFOLIO_FROM_CSV_REF: &AgentRecipe = &PORTFOLIO_FROM_CSV;

    #[test]
    fn recipe_addendum_includes_example_when_present() {
        let s = build_recipe_addendum(PORTFOLIO_FROM_CSV_REF);
        assert!(s.contains("RECIPE: PortfolioFromCsv"));
        assert!(s.contains("Example plans for this recipe"));
        assert!(s.contains("create_account"));
    }

    #[test]
    fn all_recipes_are_gold_for_v1() {
        // Stop hook for "free tier doesn't get agent mode": every
        // recipe should be gold_only until we explicitly decide
        // otherwise per-recipe.
        for r in ALL_RECIPES {
            assert!(r.gold_only, "{} must be gold-only in v1", r.id);
        }
    }
}
