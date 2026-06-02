//! Bridge between the chat dispatcher and the agent runtime.
//!
//! The chat dispatcher in [`crate::chat`] receives a user message +
//! attachments + thread context, then decides whether to:
//!
//!   (a) run the legacy single-turn rig-core chat loop (current
//!       behaviour for free tier / non-agentic prompts), OR
//!   (b) detect a recipe match and route into the agent runtime,
//!       forwarding the resulting [`crate::agent::AgentEvent`]s as
//!       [`AiStreamEvent::Agent`] events on the same SSE channel.
//!
//! This module owns path (b). It exposes [`maybe_run_agent`] —
//! a fire-and-stream entry point the chat dispatcher calls at the
//! TOP of `stream_ai_chat`, before building any rig-core agent:
//!
//! ```text
//!   if let Some(agent_handle) = maybe_run_agent(msg, attachments, env, …).await {
//!       // forward AgentEvents into the SSE channel; return early.
//!   } else {
//!       // fall through to the legacy chat loop.
//!   }
//! ```
//!
//! ## Why this layer exists separately
//!
//! Keeping the bridge thin and isolated means:
//!  - The chat dispatcher's `stream_ai_chat` stays readable — just
//!    one new conditional branch at the top.
//!  - The agent runtime stays decoupled from rig-core, chat history
//!    persistence, and SSE framing.
//!  - Tests for the bridge can mock the runtime + the channel without
//!    standing up the full chat dispatcher.
//!
//! ## Gold-tier gate
//!
//! The bridge ENFORCES the gold-tier requirement before dispatching.
//! Free / Silver users matching a recipe see a friendly "upgrade to
//! Agent Mode" event instead of the agent running. The gate lives
//! here (not in the runtime) so the runtime stays generic.

use std::sync::Arc;

use crate::agent::{
    AgentEvent, AgentPlanner, AgentRunHandle, AgentRuntime, AgentToolDispatcher,
    InMemoryAgentRuntime, PlannerAttachment, PlannerContext,
};
use crate::agent_recipes::{
    build_recipe_addendum, detect_recipe, filter_tools_for_recipe, AgentRecipe,
};
use mizan_core::truth_engine::TruthLedger;

/// Capability tier of the requesting user. Decided by the chat
/// dispatcher from the resolved subscription state; the bridge only
/// needs to know whether agent mode is unlocked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentTier {
    /// Free or Silver — agent mode is locked.
    Locked,
    /// Gold or above — agent mode unlocked.
    Unlocked,
}

/// Outcome of consulting the bridge.
pub enum BridgeDecision {
    /// No recipe matched the input. The chat dispatcher should run
    /// its normal single-turn loop.
    NotAgentic,
    /// A recipe matched but the user is on a Free/Silver tier. The
    /// chat dispatcher should emit ONE marker event ("Upgrade to
    /// unlock agent mode for X") and then either fall through to the
    /// legacy chat loop OR short-circuit with an upgrade card —
    /// caller's choice.
    LockedRecipe { recipe: &'static AgentRecipe },
    /// A recipe matched AND the user is Gold. The runtime is
    /// running; the chat dispatcher should forward events from the
    /// handle into the SSE channel and skip the legacy loop.
    Running {
        recipe: &'static AgentRecipe,
        handle: AgentRunHandle,
    },
}

/// Inputs the bridge needs to make a decision. Kept in a struct so
/// the chat dispatcher can pass everything through one call site
/// without a fat function signature.
pub struct AgentInvocation<'a> {
    pub user_message: &'a str,
    pub attachments: Vec<PlannerAttachment>,
    pub tier: AgentTier,
    pub planner: Arc<dyn AgentPlanner>,
    pub dispatcher: Arc<dyn AgentToolDispatcher>,
    pub truth_ledger: Arc<dyn TruthLedger>,
    pub user_id: Option<String>,
    pub base_currency: Option<String>,
}

/// Decide whether to run the agent + start it if so.
///
/// Returns:
///  - [`BridgeDecision::NotAgentic`] when no recipe keyword matches.
///  - [`BridgeDecision::LockedRecipe`] when a recipe matches but the
///    user's tier is below Gold — caller renders the upgrade nudge.
///  - [`BridgeDecision::Running`] with an [`AgentRunHandle`] when the
///    runtime has spawned the agent. Caller drains
///    `handle.events` into the SSE channel.
pub async fn maybe_run_agent(
    inv: AgentInvocation<'_>,
) -> Result<BridgeDecision, crate::agent::AgentError> {
    let has_attachment = !inv.attachments.is_empty();
    let recipe = match detect_recipe(inv.user_message, has_attachment) {
        Some(r) => r,
        None => return Ok(BridgeDecision::NotAgentic),
    };

    // Tier gate. Recipes are gold-only in v1 (see agent_recipes:
    // every recipe.gold_only is true and the unit test enforces it).
    if recipe.gold_only && inv.tier != AgentTier::Unlocked {
        return Ok(BridgeDecision::LockedRecipe { recipe });
    }

    let runtime = InMemoryAgentRuntime::new(
        Arc::clone(&inv.planner),
        Arc::clone(&inv.dispatcher),
        Arc::clone(&inv.truth_ledger),
    );

    // Recipe-shaped planner context: includes the user attachments
    // (so the LLM knows the CSV is in scope) + any completed-steps
    // history (empty for a fresh run; populated by re-plans).
    let context = PlannerContext {
        attachments: inv.attachments,
        user_id: inv.user_id,
        base_currency: inv.base_currency,
        completed_steps: Vec::new(),
    };

    // The recipe addendum is logged here for support visibility —
    // the actual prompt construction happens inside the planner.
    log::info!(
        "agent: starting recipe={} addendum_chars={}",
        recipe.id,
        build_recipe_addendum(recipe).len()
    );

    // Compose the goal: prepend the recipe addendum to the user
    // message so the planner LLM has both the user's intent AND
    // the recipe's domain-specific instructions in one prompt.
    let goal = format!(
        "{}\n\n--- User goal ---\n{}",
        build_recipe_addendum(recipe),
        inv.user_message
    );

    let handle = runtime.run(goal, context).await?;
    Ok(BridgeDecision::Running { recipe, handle })
}

/// Helper: wrap an [`AgentEvent`] in an [`crate::types::AiStreamEvent::Agent`]
/// envelope for SSE delivery. Public so the chat dispatcher can do
/// the wrap inline in its forwarder loop.
pub fn wrap_agent_event_for_sse(
    thread_id: &str,
    run_id: &str,
    message_id: &str,
    event: AgentEvent,
) -> crate::types::AiStreamEvent {
    crate::types::AiStreamEvent::Agent {
        thread_id: thread_id.to_string(),
        run_id: run_id.to_string(),
        message_id: message_id.to_string(),
        event,
    }
}

/// Tools the agent runtime accepts. The chat dispatcher constructs
/// this list from the recipe's allow-list + the global tool catalog
/// — kept as a free function so callers can reuse the recipe-filter
/// logic without going through the bridge.
pub fn build_recipe_tool_descriptors(
    recipe: &AgentRecipe,
    all: &[crate::agent_planner::ToolDescriptor],
) -> Vec<crate::agent_planner::ToolDescriptor> {
    filter_tools_for_recipe(recipe, all)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{AgentError, DispatchResult, PlanStep};
    use crate::agent_planner::ToolDescriptor;
    use async_trait::async_trait;
    use mizan_core::truth_engine::InMemoryTruthLedger;

    struct DummyPlanner;
    #[async_trait]
    impl AgentPlanner for DummyPlanner {
        async fn plan(&self, _g: &str, _c: &PlannerContext) -> Result<Vec<PlanStep>, AgentError> {
            Ok(vec![PlanStep {
                id: "a".to_string(),
                tool: "noop".to_string(),
                args: serde_json::json!({}),
                depends_on: vec![],
                summary: "noop".to_string(),
                verify: None,
            }])
        }
        async fn replan(
            &self,
            _g: &str,
            _c: &PlannerContext,
            _f: &str,
        ) -> Result<Vec<PlanStep>, AgentError> {
            self.plan(_g, _c).await
        }
    }

    struct DummyDispatcher;
    #[async_trait]
    impl AgentToolDispatcher for DummyDispatcher {
        async fn dispatch(
            &self,
            _t: &str,
            _a: serde_json::Value,
        ) -> Result<DispatchResult, AgentError> {
            Ok(DispatchResult {
                output: serde_json::json!({"ok": true}),
                ledger_entries: vec!["led:1".to_string()],
            })
        }
    }

    fn invocation<'a>(message: &'a str, tier: AgentTier, attached: bool) -> AgentInvocation<'a> {
        let attachments = if attached {
            vec![PlannerAttachment {
                filename: "portfolio.csv".to_string(),
                content_type: "text/csv".to_string(),
                content: "date,symbol\n2024,AAPL".to_string(),
            }]
        } else {
            vec![]
        };
        AgentInvocation {
            user_message: message,
            attachments,
            tier,
            planner: Arc::new(DummyPlanner),
            dispatcher: Arc::new(DummyDispatcher),
            truth_ledger: Arc::new(InMemoryTruthLedger::new()),
            user_id: None,
            base_currency: Some("USD".to_string()),
        }
    }

    #[tokio::test]
    async fn no_keyword_no_attachment_not_agentic() {
        let inv = invocation("what's the weather", AgentTier::Unlocked, false);
        let decision = maybe_run_agent(inv).await.unwrap();
        assert!(matches!(decision, BridgeDecision::NotAgentic));
    }

    #[tokio::test]
    async fn keyword_matches_but_user_locked_returns_locked_recipe() {
        let inv = invocation(
            "make a new portfolio and add these stocks",
            AgentTier::Locked,
            true,
        );
        let decision = maybe_run_agent(inv).await.unwrap();
        match decision {
            BridgeDecision::LockedRecipe { recipe } => {
                assert_eq!(recipe.id, "portfolio_from_csv");
            }
            _ => panic!(
                "expected LockedRecipe, got {:?}",
                std::any::type_name_of_val(&decision)
            ),
        }
    }

    #[tokio::test]
    async fn gold_user_with_csv_starts_agent_run() {
        let inv = invocation(
            "make a new portfolio and add these stocks",
            AgentTier::Unlocked,
            true,
        );
        let decision = maybe_run_agent(inv).await.unwrap();
        match decision {
            BridgeDecision::Running { recipe, mut handle } => {
                assert_eq!(recipe.id, "portfolio_from_csv");
                // Drain a few events to confirm the runtime actually
                // started; we expect a PlanReady at minimum.
                let mut saw_plan_ready = false;
                let mut saw_terminal = false;
                while let Some(ev) = handle.events.recv().await {
                    if matches!(ev, AgentEvent::PlanReady { .. }) {
                        saw_plan_ready = true;
                    }
                    if matches!(
                        ev,
                        AgentEvent::RunComplete { .. } | AgentEvent::RunAborted { .. }
                    ) {
                        saw_terminal = true;
                        break;
                    }
                }
                assert!(saw_plan_ready, "expected PlanReady event");
                assert!(saw_terminal, "expected terminal event");
            }
            _ => panic!("expected Running"),
        }
    }

    #[test]
    fn wrap_agent_event_for_sse_round_trips_through_serde() {
        let inner = AgentEvent::Verified {
            run_id: uuid::Uuid::new_v4(),
            discrepancies: vec![],
        };
        let wrapped = wrap_agent_event_for_sse("t1", "r1", "m1", inner);
        let s = serde_json::to_string(&wrapped).unwrap();
        assert!(s.contains("\"type\":\"agent\""));
        // Inner event tag is "verified" (snake_case from the
        // AgentEvent serde rename).
        assert!(s.contains("\"verified\""));
        let _: crate::types::AiStreamEvent = serde_json::from_str(&s).unwrap();
    }

    #[test]
    fn build_recipe_tool_descriptors_uses_recipe_filter() {
        let all = vec![
            ToolDescriptor {
                name: "create_account".to_string(),
                one_line_summary: "x".to_string(),
                args_schema: serde_json::json!({}),
                mutates: true,
            },
            ToolDescriptor {
                name: "irrelevant_tool".to_string(),
                one_line_summary: "x".to_string(),
                args_schema: serde_json::json!({}),
                mutates: false,
            },
        ];
        let recipe = crate::agent_recipes::PORTFOLIO_FROM_CSV;
        let filtered = build_recipe_tool_descriptors(&recipe, &all);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "create_account");
    }
}
