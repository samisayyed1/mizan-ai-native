//! AI Chat Tauri commands for streaming responses and thread management.
//!
//! Uses Tauri's IPC Channel for efficient streaming of AI events.

use std::sync::Arc;

use futures::StreamExt;
use mizan_ai::types::MessageAttachment;
use mizan_ai::{
    AiError, AiStreamEvent, ChatMessage, ChatThread, ListThreadsRequest, SendMessageRequest,
    ThreadPage,
};
use serde::{Deserialize, Serialize};
use tauri::{ipc::Channel, State};

use crate::context::ServiceContext;

use super::error::CommandResult;

/// Request for updating thread title or pinned status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateThreadRequest {
    pub id: String,
    pub title: Option<String>,
    pub is_pinned: Option<bool>,
}

/// Stream a chat message and receive AI events through a Tauri Channel.
///
/// The channel will receive `AiStreamEvent` objects:
/// - `system`: Initial event with thread_id, run_id, message_id
/// - `textDelta`: Partial text content
/// - `reasoningDelta`: Optional reasoning/thinking content
/// - `toolCall`: Tool invocation request
/// - `toolResult`: Tool execution result
/// - `error`: Error event
/// - `done`: Terminal event with final message
///
/// Returns Ok(()) when the stream completes successfully.
#[tauri::command]
pub async fn stream_ai_chat(
    context: State<'_, Arc<ServiceContext>>,
    request: SendMessageRequest,
    on_event: Channel<AiStreamEvent>,
) -> CommandResult<()> {
    let service = context.ai_chat_service();

    let mut event_stream = service.send_message(request).await?;

    // Stream events to the frontend via the Tauri channel
    while let Some(event) = event_stream.next().await {
        if let Err(e) = on_event.send(event) {
            log::error!("Failed to send AI event to channel: {}", e);
            break;
        }
    }

    Ok(())
}

// ============================================================================
// Agent runtime commands (Gold-tier autonomous mode)
// ============================================================================

/// Request payload for [`stream_agent_chat`]. Mirrors a subset of
/// [`SendMessageRequest`] — the fields the agent runtime needs to
/// kick off a recipe-driven run.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunRequest {
    /// The user's free-form goal (e.g. "make a new portfolio and add
    /// these stocks").
    pub content: String,
    /// File attachments — primarily CSVs the agent will parse.
    /// Same shape as the chat command's attachments.
    #[serde(default)]
    pub attachments: Vec<MessageAttachment>,
    /// Optional thread id to associate the agent run with. None = the
    /// chat shell creates a fresh thread for the run.
    pub thread_id: Option<String>,
    /// Force a specific recipe by id. When None, the runtime picks
    /// the best-matching recipe via `detect_recipe`. UI exposes the
    /// override so power users can rerun a specific recipe deliberately.
    pub recipe_id: Option<String>,
}

/// Stream an autonomous agent run.
///
/// Distinct from [`stream_ai_chat`] — this command bypasses the legacy
/// single-turn rig-core loop and instead spins up an
/// [`mizan_ai::agent::InMemoryAgentRuntime`] that:
///
///   1. Calls the planner LLM to decompose the goal into PlanSteps.
///   2. Executes each step autonomously via the ToolSetDispatcher.
///   3. Verifies the result.
///   4. Reports back via [`AiStreamEvent::Agent`] events streamed on
///      the same Tauri Channel.
///
/// Gold-tier-gated: free / silver users get an immediate "upgrade"
/// event and the command returns. Tier check uses the existing
/// `connect_service().capabilities()` surface.
///
/// LLM planner: production deployments wire this through the user's
/// active AI provider (Mizan Connect / OpenAI / Anthropic / etc.).
/// The v1 implementation here uses a placeholder planner that emits
/// a hardcoded plan for the matched recipe — enough to demonstrate
/// the full event stream to the frontend without burning LLM credits
/// during development. The real planner integration lands in a
/// follow-on once the chat dispatcher's provider router is exposed
/// as a shareable async closure.
#[tauri::command]
pub async fn stream_agent_chat(
    context: State<'_, Arc<ServiceContext>>,
    request: AgentRunRequest,
    on_event: Channel<AiStreamEvent>,
) -> CommandResult<()> {
    use mizan_ai::agent::{
        AgentEvent, AgentPlanner, AgentRuntime, AgentToolDispatcher, InMemoryAgentRuntime,
        PlannerAttachment, PlannerContext,
    };
    use mizan_ai::agent_chat_bridge::{wrap_agent_event_for_sse, AgentTier};
    use mizan_ai::agent_dispatcher::ToolSetDispatcher;
    use mizan_ai::agent_recipes::{detect_recipe, ALL_RECIPES, PORTFOLIO_FROM_CSV};
    use uuid::Uuid;

    let thread_id = request
        .thread_id
        .clone()
        .unwrap_or_else(|| Uuid::now_v7().to_string());
    let run_id = Uuid::now_v7().to_string();
    let message_id = Uuid::now_v7().to_string();

    // Emit a system event first so the frontend can correlate.
    let _ = on_event.send(AiStreamEvent::system(&thread_id, &run_id, &message_id));

    // ── Tier gate ───────────────────────────────────────────────────
    // The connect service surfaces the user's subscription state. We
    // treat Gold + Lifetime as agent-unlocked; everyone else (Free /
    // Silver / no-sub) sees the upgrade nudge.
    match resolve_agent_tier(&context).await {
        AgentTier::Unlocked => {}
        AgentTier::Locked => {
            let _ = on_event.send(AiStreamEvent::error(
                &thread_id,
                &run_id,
                Some(&message_id),
                "agent_tier_locked",
                "Agent Mode is a Gold-tier feature. Sign in and upgrade to unlock autonomous multi-step flows.",
            ));
            return Ok(());
        }
    }

    // ── Recipe selection ────────────────────────────────────────────
    // The hard-coded RecipePlanner only emits sensible plans for the
    // handful of intents listed in `ALL_RECIPES`. When the user types
    // a free-form add-intent that doesn't match a recipe (e.g. "add
    // 10 oz of gold" without a CSV, or "delete my Wise account"),
    // returning a canned plan would be misleading — the agent
    // would emit "Create the US Stocks portfolio · Parse the CSV ·
    // Read back" steps that aren't related to the user's goal. Emit
    // a distinct `agent_no_recipe_match` error code instead so the
    // frontend can route the request to the regular chat path
    // (`streamAiChat`), where the full mutation tool surface
    // (create_account / record_activity / delete_* / …) is available.
    let has_attachment = !request.attachments.is_empty();
    let recipe = if let Some(forced_id) = request.recipe_id.as_deref() {
        ALL_RECIPES
            .iter()
            .find(|r| r.id == forced_id)
            .cloned()
            .unwrap_or(PORTFOLIO_FROM_CSV)
    } else {
        match detect_recipe(&request.content, has_attachment) {
            Some(r) => r.clone(),
            None => {
                let _ = on_event.send(AiStreamEvent::error(
                    &thread_id,
                    &run_id,
                    Some(&message_id),
                    "agent_no_recipe_match",
                    "No agent recipe matched this goal — routing to regular chat.",
                ));
                return Ok(());
            }
        }
    };

    // ── Planner ─────────────────────────────────────────────────────
    // v1 uses a recipe-driven hardcoded planner so the full streaming
    // experience works without an LLM round-trip. The agent runtime
    // doesn't care that the plan is hardcoded — it still topologically
    // sorts, executes, verifies, and reports the same way it would
    // with an LLM-emitted plan. Swap this for the real LlmPlanner
    // once the chat dispatcher's provider router is callable as a
    // standalone closure (queued).
    let planner: Arc<dyn AgentPlanner> = Arc::new(RecipePlanner {
        recipe_id: recipe.id.to_string(),
    });

    // ── Dispatcher + ledger ─────────────────────────────────────────
    let ai_chat_service = context.ai_chat_service();
    let _ = ai_chat_service; // hold a ref — currently unused, future LLM integration uses it
    let env = context.ai_env();
    let base_currency = context.get_base_currency();
    let tool_set = Arc::new(mizan_ai::tools::ToolSet::new(env, base_currency.clone()));
    let dispatcher: Arc<dyn AgentToolDispatcher> = Arc::new(ToolSetDispatcher::new(tool_set));
    let truth_ledger = context.truth_ledger();

    let runtime = InMemoryAgentRuntime::new(planner, dispatcher, truth_ledger);

    let attachments: Vec<PlannerAttachment> = request
        .attachments
        .iter()
        .map(|a| PlannerAttachment {
            filename: a.name.clone(),
            content_type: a.content_type.clone(),
            content: a.data.clone(),
        })
        .collect();

    let context_for_run = PlannerContext {
        attachments,
        user_id: None,
        base_currency: Some(base_currency),
        completed_steps: Vec::new(),
    };

    let recipe_addendum = mizan_ai::agent_recipes::build_recipe_addendum(&recipe);
    let goal = format!(
        "{}\n\n--- User goal ---\n{}",
        recipe_addendum, request.content
    );

    // ── Run + stream ────────────────────────────────────────────────
    let handle = match runtime.run(goal, context_for_run).await {
        Ok(h) => h,
        Err(e) => {
            let _ = on_event.send(AiStreamEvent::error(
                &thread_id,
                &run_id,
                Some(&message_id),
                "agent_plan_failed",
                &format!("Agent failed to plan: {e}"),
            ));
            return Ok(());
        }
    };

    let mut events = handle.events;
    while let Some(agent_event) = events.recv().await {
        let is_terminal = matches!(
            agent_event,
            AgentEvent::RunComplete { .. } | AgentEvent::RunAborted { .. }
        );
        let wrapped = wrap_agent_event_for_sse(&thread_id, &run_id, &message_id, agent_event);
        if on_event.send(wrapped).is_err() {
            // Channel closed — frontend gave up on the stream.
            log::warn!("agent stream channel closed mid-run");
            break;
        }
        if is_terminal {
            break;
        }
    }

    Ok(())
}

/// Resolve the user's agent tier from the connect service capabilities.
///
/// Routes through `resolve_entitlements`, which already honours the
/// `MIZAN_DEMO_MODE=1` + `MIZAN_ALLOW_PRODUCTION=1` override (see
/// `commands::entitlements`). Previously this called
/// `has_broker_sync()` directly, which bypassed the demo override and
/// left Agent Mode showing "Sign in and upgrade" inside the pitch
/// flow even with both env vars set. Now Agent Mode unlocks under the
/// same gate every other Gold-tier surface uses, so the AI write
/// tools surface (delete_account / update_holding / record_activities
/// / …) actually fires for the demo user.
async fn resolve_agent_tier(
    context: &Arc<ServiceContext>,
) -> mizan_ai::agent_chat_bridge::AgentTier {
    use mizan_ai::agent_chat_bridge::AgentTier;
    let entitlements = crate::commands::entitlements::resolve_entitlements(context).await;
    if entitlements.broker_sync {
        AgentTier::Unlocked
    } else {
        AgentTier::Locked
    }
}

/// v1 hardcoded planner — emits a canned plan per recipe id so the
/// agent runtime can demonstrate the full event stream to the
/// frontend without requiring an LLM round-trip.
///
/// Replace with `mizan_ai::agent_planner::PromptBasedPlanner` (already
/// implemented) once the chat dispatcher's provider router is exposed
/// as a shareable async closure. The agent_planner module + its tests
/// are already production-ready; only the closure plumbing is missing.
struct RecipePlanner {
    recipe_id: String,
}

#[async_trait::async_trait]
impl mizan_ai::agent::AgentPlanner for RecipePlanner {
    async fn plan(
        &self,
        _goal: &str,
        _context: &mizan_ai::agent::PlannerContext,
    ) -> Result<Vec<mizan_ai::agent::PlanStep>, mizan_ai::agent::AgentError> {
        use mizan_ai::agent::PlanStep;
        Ok(match self.recipe_id.as_str() {
            "portfolio_from_csv" => vec![
                PlanStep {
                    id: "create".into(),
                    tool: "create_account".into(),
                    args: serde_json::json!({
                        "name": "US Stocks",
                        "currency": "USD",
                        "accountType": "SECURITIES"
                    }),
                    depends_on: vec![],
                    summary: "Create the US Stocks portfolio account".into(),
                    verify: None,
                },
                PlanStep {
                    id: "parse".into(),
                    tool: "parse_csv".into(),
                    args: serde_json::json!({"csvContent": "(populated from attachment by runtime)"}),
                    depends_on: vec!["create".into()],
                    summary: "Parse the attached CSV".into(),
                    verify: None,
                },
                PlanStep {
                    id: "summary".into(),
                    tool: "query_account_summary".into(),
                    args: serde_json::json!({"accountId": "<create>"}),
                    depends_on: vec!["parse".into()],
                    summary: "Read back the new account state".into(),
                    verify: None,
                },
            ],
            _ => vec![PlanStep {
                id: "noop".into(),
                tool: "abort_with_message".into(),
                args: serde_json::json!({
                    "reason": format!("Recipe '{}' has no v1 hardcoded plan yet — wire LlmPlanner.", self.recipe_id)
                }),
                depends_on: vec![],
                summary: "Recipe planner not yet wired".into(),
                verify: None,
            }],
        })
    }

    async fn replan(
        &self,
        goal: &str,
        ctx: &mizan_ai::agent::PlannerContext,
        _failure: &str,
    ) -> Result<Vec<mizan_ai::agent::PlanStep>, mizan_ai::agent::AgentError> {
        self.plan(goal, ctx).await
    }
}

// ============================================================================
// Thread Management Commands
// ============================================================================

/// List all chat threads with cursor-based pagination and optional search.
///
/// Returns a `ThreadPage` with threads, next_cursor, and has_more flag.
#[tauri::command]
pub async fn list_ai_threads(
    context: State<'_, Arc<ServiceContext>>,
    cursor: Option<String>,
    limit: Option<u32>,
    search: Option<String>,
) -> CommandResult<ThreadPage> {
    let service = context.ai_chat_service();
    let request = ListThreadsRequest {
        cursor,
        limit,
        search,
    };
    let page = service.list_threads_paginated(&request)?;
    Ok(page)
}

/// Get a single chat thread by ID.
#[tauri::command]
pub async fn get_ai_thread(
    context: State<'_, Arc<ServiceContext>>,
    thread_id: String,
) -> CommandResult<Option<ChatThread>> {
    let service = context.ai_chat_service();
    let thread = service.get_thread(&thread_id)?;
    Ok(thread)
}

/// Get all messages for a chat thread.
#[tauri::command]
pub async fn get_ai_thread_messages(
    context: State<'_, Arc<ServiceContext>>,
    thread_id: String,
) -> CommandResult<Vec<ChatMessage>> {
    let service = context.ai_chat_service();
    let messages = service.get_messages(&thread_id)?;
    Ok(messages)
}

/// Update a chat thread's title and/or pinned status.
#[tauri::command]
pub async fn update_ai_thread(
    context: State<'_, Arc<ServiceContext>>,
    request: UpdateThreadRequest,
) -> CommandResult<ChatThread> {
    let service = context.ai_chat_service();

    // Update title if provided
    if let Some(title) = request.title {
        service.update_thread_title(&request.id, title).await?;
    }

    // Update pinned status if provided
    if let Some(is_pinned) = request.is_pinned {
        service.update_thread_pinned(&request.id, is_pinned).await?;
    }

    // Get updated thread
    let thread = service
        .get_thread(&request.id)?
        .ok_or_else(|| AiError::ThreadNotFound(request.id.clone()))?;
    Ok(thread)
}

/// Delete a chat thread and all its messages.
#[tauri::command]
pub async fn delete_ai_thread(
    context: State<'_, Arc<ServiceContext>>,
    thread_id: String,
) -> CommandResult<()> {
    let service = context.ai_chat_service();
    service.delete_thread(&thread_id).await?;
    Ok(())
}

// ============================================================================
// Tag Management Commands
// ============================================================================

/// Add a tag to a thread.
#[tauri::command]
pub async fn add_ai_thread_tag(
    _context: State<'_, Arc<ServiceContext>>,
    _thread_id: String,
    _tag: String,
) -> CommandResult<()> {
    // TODO: Add tag support to ChatService
    Ok(())
}

/// Remove a tag from a thread.
#[tauri::command]
pub async fn remove_ai_thread_tag(
    _context: State<'_, Arc<ServiceContext>>,
    _thread_id: String,
    _tag: String,
) -> CommandResult<()> {
    // TODO: Add tag support to ChatService
    Ok(())
}

/// Get all tags for a thread.
#[tauri::command]
pub async fn get_ai_thread_tags(
    context: State<'_, Arc<ServiceContext>>,
    thread_id: String,
) -> CommandResult<Vec<String>> {
    let service = context.ai_chat_service();
    let tags = service
        .get_thread(&thread_id)?
        .map(|t| t.tags)
        .unwrap_or_default();
    Ok(tags)
}

// ============================================================================
// Tool Result Update Command
// ============================================================================

/// Request for updating a tool result in a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateToolResultRequest {
    /// The thread ID containing the message with the tool result.
    pub thread_id: String,
    /// The tool call ID to update.
    pub tool_call_id: String,
    /// JSON patch to merge into the tool result data.
    pub result_patch: serde_json::Value,
}

/// Update a tool result in a message by merging a patch into the result data.
///
/// This is used by mutation tool UIs (e.g., record_activity) to persist
/// submission state. After the user confirms and the backend operation succeeds,
/// the frontend calls this to store metadata like created_activity_id.
#[tauri::command]
pub async fn update_tool_result(
    context: State<'_, Arc<ServiceContext>>,
    request: UpdateToolResultRequest,
) -> CommandResult<ChatMessage> {
    let service = context.ai_chat_service();
    let message = service
        .update_tool_result(
            &request.thread_id,
            &request.tool_call_id,
            request.result_patch,
        )
        .await?;
    Ok(message)
}
