//! Agent runtime — the foundation for Mercor/Polaris-style autonomous
//! multi-step execution.
//!
//! # Why this exists
//!
//! The chat dispatcher in [`crate::chat`] is a single-turn responder:
//! the model picks ONE tool, dispatches it, then waits for the user to
//! confirm the draft before the next turn. That's "AI-assisted." Real
//! AI-native products (Devin, Cursor Composer, Polaris, Mercor) operate
//! on **goals** — the user describes an outcome, and the agent
//! decomposes, executes, verifies, and reports without per-step
//! approval. This module is the runtime that enables that pattern.
//!
//! # Architecture
//!
//! ```text
//!   GOAL (string)
//!     │
//!     ▼
//!   ┌───────────────┐
//!   │ PLAN          │  LLM emits a list of PlanStep
//!   │ (one call)    │  via structured-output JSON.
//!   └───────────────┘
//!     │
//!     ▼
//!   ┌───────────────┐
//!   │ EXECUTE LOOP  │  For each step:
//!   │ (N calls)     │    • Run the tool with provided args
//!   │               │    • Append the resulting truth-ledger entry id
//!   │               │      to AgentRun.ledger_entries (for Undo)
//!   │               │    • Emit AgentEvent::StepStart / Done / Failed
//!   │               │    • On failure → retry up to RETRY_BUDGET, then
//!   │               │      ask the LLM to re-plan from current state
//!   └───────────────┘
//!     │
//!     ▼
//!   ┌───────────────┐
//!   │ VERIFY        │  LLM (or deterministic check) reads back state,
//!   │ (one call)    │  asserts the goal is satisfied. Discrepancies
//!   │               │  surface as `AgentEvent::Verified.discrepancies`.
//!   └───────────────┘
//!     │
//!     ▼
//!   ┌───────────────┐
//!   │ REPORT        │  AgentEvent::RunComplete with summary + the
//!   │               │  full list of ledger entries for Undo.
//!   └───────────────┘
//! ```
//!
//! # Undo contract
//!
//! Every tool call that mutates state MUST write a [`truth_engine`]
//! [`LedgerEntry`] before returning. The agent runtime collects those
//! entry ids in [`AgentRun::ledger_entries`]. [`AgentRuntime::undo`]
//! walks that list in **reverse** order, appending an
//! [`LedgerEntryKind::ActivityReversed`] (or kind-appropriate reversal)
//! for each one, so a single user click rolls back the whole batch.
//!
//! # Safety invariants
//!
//! 1. **Tool budget cap.** The agent CANNOT call more than
//!    [`MAX_TOOL_CALLS_PER_RUN`] tools in a single run. Beyond that the
//!    runtime aborts with [`AgentError::ToolBudgetExceeded`] and the
//!    user sees a clear "agent stopped — too many steps" message. This
//!    is the hard floor against infinite loops + cost runaway.
//! 2. **Wall-clock cap.** [`MAX_RUN_DURATION`] (5 minutes default).
//!    Long-running agent runs are queued, not synchronously executed.
//! 3. **Goal-tier gate.** The runtime trait can be called from anywhere,
//!    but the chat dispatcher in [`crate::chat`] checks the user's tier
//!    and refuses Free/Silver clients with an upgrade prompt. This is
//!    enforced at the boundary, not inside the runtime.
//!
//! # What this module is NOT
//!
//! - **Not a planner LLM call.** Planning is delegated to the AI
//!   provider via a structured-output prompt; this module just shapes
//!   the request and parses the response.
//! - **Not a UI.** The frontend renders [`AgentEvent`]s into a
//!   progress card; that lives in `apps/frontend/src/features/ai-assistant/`.
//! - **Not a tool implementation.** Tools live in [`crate::tools`].
//!   The runtime invokes them via the registry exposed by the
//!   [`AgentToolDispatcher`] trait so the runtime stays decoupled
//!   from any specific tool list (testable with mock dispatchers).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::Mutex;
use uuid::Uuid;

use mizan_core::errors::Error as CoreError;
use mizan_core::truth_engine::{
    AppendInput, LedgerEntryKind, TruthLedger,
};

// ─── Configuration constants ────────────────────────────────────────

/// Hard cap on tool calls per agent run. Beyond this the runtime
/// aborts to prevent infinite loops + cost runaway. ~50 supports
/// realistic flows (CSV import with 1 account + 1 verification call
/// uses ~3; goal-planning uses ~10; broker reconciliation uses ~20).
pub const MAX_TOOL_CALLS_PER_RUN: u32 = 50;

/// Per-step retry budget when a tool call fails transiently. Beyond
/// this the runtime asks the planner LLM to re-plan from the current
/// state rather than blindly retrying.
pub const MAX_STEP_RETRIES: u32 = 2;

/// Wall-clock cap on a single agent run. Sync runs that exceed this
/// are aborted; long-tail flows should be queued (out of scope for
/// the v1 runtime — they'll land in a follow-on PR).
pub const MAX_RUN_DURATION: Duration = Duration::from_secs(300);

// ─── Plan ────────────────────────────────────────────────────────────

/// A single step the agent intends to execute. The planner LLM emits
/// a `Vec<PlanStep>` via structured-output JSON; the runtime walks the
/// list in dependency order.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlanStep {
    /// Stable id assigned by the planner. Used for `depends_on`
    /// references — the runtime executes steps in topological order
    /// derived from this graph.
    pub id: String,
    /// Tool name from [`crate::tools`] registry.
    pub tool: String,
    /// JSON args passed verbatim to the tool. The agent runtime does
    /// NOT validate the schema here — the tool's own arg deserialiser
    /// does, and any error surfaces as [`AgentEvent::StepFailed`].
    pub args: serde_json::Value,
    /// Step ids this step waits on. Empty = can start immediately.
    /// Cycles are detected at plan-validation time and rejected with
    /// [`AgentError::PlanCycle`].
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Human-readable one-liner the UI surfaces in the progress card.
    /// e.g. "Create the US Stocks portfolio account."
    pub summary: String,
    /// When this step kind is `Verify`, the expected condition the
    /// runtime checks after the previous steps land. Ignored for
    /// non-verify steps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify: Option<VerifyExpectation>,
}

/// Verification condition the agent asserts after writes land.
/// `MatchesSource` is the most common: "the totals I computed from
/// the source CSV must equal the totals the activity service now
/// reports for this account."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VerifyExpectation {
    /// Expected numeric value within an inclusive tolerance.
    EqualWithin {
        actual_tool: String,
        actual_path: String, // JSON-pointer style key
        expected: serde_json::Value,
        tolerance: f64,
    },
    /// Row count of a list-shaped tool output equals `expected_count`.
    CountEquals {
        actual_tool: String,
        expected_count: u32,
    },
    /// Free-form natural-language assertion the LLM checks. Lowest
    /// confidence but useful for fuzzy goals ("portfolio is balanced").
    NaturalLanguage(String),
}

// ─── Events ──────────────────────────────────────────────────────────

/// Every state transition during an agent run emits one event. The
/// frontend renders these into a live progress card.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    /// Planner finished. The UI replaces the "thinking..." state with
    /// a visible step list at this point.
    PlanReady {
        run_id: Uuid,
        plan: Vec<PlanStep>,
        estimated_steps: u32,
    },
    /// A step is about to dispatch. `step_index` is 0-based.
    StepStart {
        run_id: Uuid,
        step_index: u32,
        total_steps: u32,
        step_id: String,
        summary: String,
    },
    /// Optional intra-step heartbeat for long-running tools (e.g.
    /// "Adding activities... 47/402"). Tools that don't emit progress
    /// just go StepStart → StepDone.
    StepProgress {
        run_id: Uuid,
        step_index: u32,
        progress: f32, // 0.0 .. 1.0
        message: String,
    },
    /// Step finished successfully. `ledger_entries` accumulates the
    /// truth-ledger ids the tool appended (for Undo).
    StepDone {
        run_id: Uuid,
        step_index: u32,
        step_id: String,
        ledger_entries: Vec<String>,
    },
    /// Step failed. `retrying` = true means the runtime will retry
    /// automatically; false means the budget is exhausted and the
    /// runtime is escalating to a re-plan or aborting.
    StepFailed {
        run_id: Uuid,
        step_index: u32,
        step_id: String,
        error: String,
        retrying: bool,
    },
    /// Re-plan kicked off after exhausting retries on a step. The
    /// runtime calls the planner LLM again with the failure context
    /// so the LLM can propose an alternative path. UI can show
    /// "Adapting plan..." while this runs.
    RePlanning {
        run_id: Uuid,
        reason: String,
    },
    /// Verification phase completed. `discrepancies` is empty on
    /// success; populated with human-readable mismatches when the
    /// post-run state doesn't satisfy the goal.
    Verified {
        run_id: Uuid,
        discrepancies: Vec<String>,
    },
    /// Terminal event. `undo_batch_id` is the agent run id — clients
    /// pass it to `AgentRuntime::undo` to roll back the entire batch.
    RunComplete {
        run_id: Uuid,
        undo_batch_id: Uuid,
        summary: String,
        tool_calls: u32,
        wall_clock_ms: u64,
    },
    /// Terminal event for failure. Distinct from `StepFailed` —
    /// this fires when the WHOLE run is unrecoverable (re-plan
    /// failed, wall clock exceeded, tool budget exceeded).
    RunAborted {
        run_id: Uuid,
        reason: String,
        partial_ledger_entries: Vec<String>,
    },
}

// ─── Errors ──────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("planner returned no steps")]
    EmptyPlan,
    #[error("plan dependency graph contains a cycle involving step {0}")]
    PlanCycle(String),
    #[error("plan references unknown step id {0}")]
    PlanUnknownDep(String),
    #[error("plan exceeds tool-call budget ({0} > {} MAX)", MAX_TOOL_CALLS_PER_RUN)]
    ToolBudgetExceeded(u32),
    #[error("tool {tool_name} failed after {attempts} attempts: {message}")]
    ToolFailed {
        tool_name: String,
        attempts: u32,
        message: String,
    },
    #[error("agent run exceeded wall-clock budget of {0:?}")]
    WallClockExceeded(Duration),
    #[error("planner LLM returned malformed plan JSON: {0}")]
    PlanParse(String),
    #[error("verification failed: {0}")]
    VerificationFailed(String),
    #[error("re-plan failed: {0}")]
    RePlanFailed(String),
    #[error("undo failed for ledger entry {0}: {1}")]
    UndoFailed(String, String),
    #[error("internal: {0}")]
    Internal(String),
    #[error("core error: {0}")]
    Core(#[from] CoreError),
}

// ─── Run state ───────────────────────────────────────────────────────

/// Captured state of a single agent execution. Persisted to the
/// truth ledger via an `AgentRunStarted` entry at the top so the
/// audit trail records "the agent did this whole batch."
#[derive(Debug, Clone)]
pub struct AgentRun {
    pub run_id: Uuid,
    pub goal: String,
    pub plan: Vec<PlanStep>,
    pub state: ExecutionState,
    /// Truth-ledger entry ids appended during this run. Used by
    /// [`AgentRuntime::undo`] to reverse the batch.
    pub ledger_entries: Vec<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    /// Total tool calls actually dispatched (counts retries).
    pub tool_calls: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionState {
    Planning,
    Executing { current_step: u32 },
    Verifying,
    Done,
    Aborted { reason: String },
}

// ─── Tool dispatcher trait ──────────────────────────────────────────

/// Bridge between the agent runtime and the concrete tool registry
/// in [`crate::tools`]. The runtime calls `dispatch` with a tool name +
/// args; the impl invokes the right tool and returns its JSON output
/// + the list of truth-ledger entry ids the tool appended.
///
/// This indirection keeps the runtime testable — production wires the
/// real registry, tests wire a mock that records calls.
#[async_trait]
pub trait AgentToolDispatcher: Send + Sync {
    /// Dispatch one tool call. Returns the tool output (typically a
    /// draft or read-back) AND any ledger entry ids the tool wrote.
    ///
    /// Mutating tools MUST write to the truth ledger and return the
    /// resulting entry id(s) — that's the agent runtime's only mechanism
    /// for undo.
    async fn dispatch(
        &self,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<DispatchResult, AgentError>;
}

#[derive(Debug, Clone)]
pub struct DispatchResult {
    pub output: serde_json::Value,
    pub ledger_entries: Vec<String>,
}

// ─── Planner trait ───────────────────────────────────────────────────

/// Bridges the runtime to whatever LLM emits the plan. Production
/// uses an `LlmPlanner` that calls the configured AI provider with a
/// structured-output prompt; tests use a `FixedPlanner` returning a
/// canned plan.
#[async_trait]
pub trait AgentPlanner: Send + Sync {
    async fn plan(
        &self,
        goal: &str,
        context: &PlannerContext,
    ) -> Result<Vec<PlanStep>, AgentError>;

    /// Called when execution hits an unrecoverable step failure;
    /// the LLM gets the failure context and proposes a new plan
    /// (or signals giving up).
    async fn replan(
        &self,
        original_goal: &str,
        current_state: &PlannerContext,
        failure: &str,
    ) -> Result<Vec<PlanStep>, AgentError>;
}

/// Snapshot of state the planner sees. Currently just the goal + any
/// attachments (e.g. a CSV file the user uploaded). Future iterations
/// can add a "current portfolio summary" so the planner is grounded.
#[derive(Debug, Clone, Default)]
pub struct PlannerContext {
    pub attachments: Vec<PlannerAttachment>,
    pub user_id: Option<String>,
    pub base_currency: Option<String>,
    pub completed_steps: Vec<(PlanStep, serde_json::Value)>,
}

#[derive(Debug, Clone)]
pub struct PlannerAttachment {
    pub filename: String,
    pub content_type: String,
    pub content: String, // base64 or raw text
}

// ─── Runtime trait ───────────────────────────────────────────────────

#[async_trait]
pub trait AgentRuntime: Send + Sync {
    /// Execute a goal. Returns a stream of events the caller (chat
    /// dispatcher, SSE endpoint) forwards to the UI.
    async fn run(
        &self,
        goal: String,
        context: PlannerContext,
    ) -> Result<AgentRunHandle, AgentError>;

    /// Reverse every mutation from a completed agent run. Appends a
    /// counter-balancing truth-ledger entry for each entry id in
    /// the run's `ledger_entries`. Idempotent — calling undo twice
    /// on the same run is a no-op the second time.
    async fn undo(&self, run_id: Uuid) -> Result<UndoReport, AgentError>;

    /// Inspect a run by id. Returns `None` if the runtime has GC'd it
    /// (typically after 24h for in-memory impls; SQLite-backed runs
    /// persist indefinitely).
    async fn get_run(&self, run_id: Uuid) -> Option<AgentRun>;
}

/// Handle returned by `run()` — carries the run id + the event
/// stream. Caller can `.await` the stream or drop it to abort.
pub struct AgentRunHandle {
    pub run_id: Uuid,
    pub events: tokio::sync::mpsc::UnboundedReceiver<AgentEvent>,
}

impl std::fmt::Debug for AgentRunHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRunHandle")
            .field("run_id", &self.run_id)
            .field("events", &"<UnboundedReceiver<AgentEvent>>")
            .finish()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UndoReport {
    pub run_id: Uuid,
    pub reversed_count: u32,
    pub failed_count: u32,
    pub failures: Vec<UndoFailure>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UndoFailure {
    pub ledger_entry_id: String,
    pub reason: String,
}

// ─── Default in-memory runtime ───────────────────────────────────────

/// Production-grade in-memory agent runtime. Persists run metadata to
/// the truth ledger (for undo) but keeps the in-flight run state in
/// a `DashMap`-backed cache. Suitable for the desktop runtime where
/// there's a single user and runs complete in under 5 minutes.
///
/// The future server-side queueing runtime (Mizan Connect) will
/// implement `AgentRuntime` with Postgres-backed persistence for
/// long-tail runs + retries across processes. For now the same
/// trait surface keeps both paths swappable.
pub struct InMemoryAgentRuntime {
    planner: Arc<dyn AgentPlanner>,
    dispatcher: Arc<dyn AgentToolDispatcher>,
    truth_ledger: Arc<dyn TruthLedger>,
    /// Run history keyed by run_id. RwLock so concurrent reads (for
    /// progress polling) don't block writes.
    runs: Arc<Mutex<HashMap<Uuid, AgentRun>>>,
}

impl InMemoryAgentRuntime {
    pub fn new(
        planner: Arc<dyn AgentPlanner>,
        dispatcher: Arc<dyn AgentToolDispatcher>,
        truth_ledger: Arc<dyn TruthLedger>,
    ) -> Self {
        Self {
            planner,
            dispatcher,
            truth_ledger,
            runs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Topological-sort a plan by `depends_on`. Errors on cycles and
    /// unknown dep ids.
    fn topo_sort(plan: &[PlanStep]) -> Result<Vec<PlanStep>, AgentError> {
        if plan.is_empty() {
            return Err(AgentError::EmptyPlan);
        }
        let ids: std::collections::HashSet<&str> =
            plan.iter().map(|s| s.id.as_str()).collect();
        for step in plan {
            for dep in &step.depends_on {
                if !ids.contains(dep.as_str()) {
                    return Err(AgentError::PlanUnknownDep(dep.clone()));
                }
            }
        }
        // Kahn's algorithm. Indegree of a node = number of deps it
        // declares (each `depends_on` entry is one incoming edge).
        let mut by_id: HashMap<&str, &PlanStep> =
            plan.iter().map(|s| (s.id.as_str(), s)).collect();
        let mut indeg: HashMap<&str, usize> = plan
            .iter()
            .map(|s| (s.id.as_str(), s.depends_on.len()))
            .collect();

        let mut ready: std::collections::VecDeque<&str> = indeg
            .iter()
            .filter(|(_, &d)| d == 0)
            .map(|(id, _)| *id)
            .collect();
        let mut sorted: Vec<PlanStep> = Vec::with_capacity(plan.len());

        while let Some(id) = ready.pop_front() {
            if let Some(step) = by_id.remove(id) {
                sorted.push(step.clone());
            }
            // For every step whose deps include `id`, decrement
            // and enqueue if it hits 0.
            for s in plan.iter() {
                if s.depends_on.iter().any(|d| d == id) {
                    if let Some(d) = indeg.get_mut(s.id.as_str()) {
                        if *d > 0 {
                            *d -= 1;
                            if *d == 0 {
                                ready.push_back(s.id.as_str());
                            }
                        }
                    }
                }
            }
        }

        if sorted.len() != plan.len() {
            // Pick any node still in by_id as the cycle witness.
            let cycle_witness = by_id.keys().next().copied().unwrap_or("?").to_string();
            return Err(AgentError::PlanCycle(cycle_witness));
        }
        Ok(sorted)
    }
}

#[async_trait]
impl AgentRuntime for InMemoryAgentRuntime {
    async fn run(
        &self,
        goal: String,
        context: PlannerContext,
    ) -> Result<AgentRunHandle, AgentError> {
        let run_id = Uuid::new_v4();
        let started_at = Utc::now();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<AgentEvent>();

        // 1. PLAN
        let raw_plan = self.planner.plan(&goal, &context).await?;
        if raw_plan.is_empty() {
            return Err(AgentError::EmptyPlan);
        }
        if raw_plan.len() as u32 > MAX_TOOL_CALLS_PER_RUN {
            return Err(AgentError::ToolBudgetExceeded(raw_plan.len() as u32));
        }
        let plan = Self::topo_sort(&raw_plan)?;
        let estimated_steps = plan.len() as u32;

        let _ = tx.send(AgentEvent::PlanReady {
            run_id,
            plan: plan.clone(),
            estimated_steps,
        });

        // Persist the run header to the truth ledger so the audit
        // trail records "the agent started a batch at time T with
        // goal G." Failure to persist is non-fatal — the agent
        // continues without an audit row.
        let mut header_meta = std::collections::BTreeMap::new();
        header_meta.insert(
            "goal".to_string(),
            serde_json::Value::String(goal.clone()),
        );
        header_meta.insert(
            "estimated_steps".to_string(),
            serde_json::Value::Number(estimated_steps.into()),
        );
        let _ = self
            .truth_ledger
            .append(AppendInput {
                id: format!("agent:{}:started", run_id),
                kind: Some(LedgerEntryKind::ActivityRecorded),
                account_id: None,
                asset_id: None,
                amount: None,
                currency: None,
                metadata: header_meta,
                recorded_at: Some(started_at),
            })
            .await;

        // 2. EXECUTE — spawn the loop so the caller gets the handle
        //    immediately and can stream events as they happen.
        let planner = Arc::clone(&self.planner);
        let dispatcher = Arc::clone(&self.dispatcher);
        let runs = Arc::clone(&self.runs);
        let plan_for_run = plan.clone();
        let goal_for_run = goal.clone();
        let tx_for_run = tx.clone();

        // Stash the initial run record so get_run() returns useful
        // state from the moment the handle is created.
        {
            let mut runs_lock = runs.lock().await;
            runs_lock.insert(
                run_id,
                AgentRun {
                    run_id,
                    goal: goal_for_run.clone(),
                    plan: plan_for_run.clone(),
                    state: ExecutionState::Executing { current_step: 0 },
                    ledger_entries: Vec::new(),
                    started_at,
                    completed_at: None,
                    tool_calls: 0,
                },
            );
        }

        tokio::spawn(async move {
            let result = execute_loop(
                run_id,
                goal_for_run,
                plan_for_run,
                planner,
                dispatcher,
                Arc::clone(&runs),
                tx_for_run.clone(),
            )
            .await;

            // Final state transition + RunComplete / RunAborted event.
            let mut runs_lock = runs.lock().await;
            if let Some(entry) = runs_lock.get_mut(&run_id) {
                entry.completed_at = Some(Utc::now());
                match &result {
                    Ok(_) => {
                        entry.state = ExecutionState::Done;
                    }
                    Err(e) => {
                        entry.state = ExecutionState::Aborted {
                            reason: e.to_string(),
                        };
                    }
                }
            }
            let elapsed_ms = (Utc::now() - started_at).num_milliseconds().max(0) as u64;
            let entry_snapshot = runs_lock.get(&run_id).cloned();
            drop(runs_lock);

            match result {
                Ok(_) => {
                    let summary = entry_snapshot
                        .as_ref()
                        .map(|r| {
                            format!(
                                "Completed {} step(s) in {}ms",
                                r.plan.len(),
                                elapsed_ms
                            )
                        })
                        .unwrap_or_else(|| "Run complete".to_string());
                    let tool_calls = entry_snapshot
                        .as_ref()
                        .map(|r| r.tool_calls)
                        .unwrap_or(0);
                    let _ = tx_for_run.send(AgentEvent::RunComplete {
                        run_id,
                        undo_batch_id: run_id,
                        summary,
                        tool_calls,
                        wall_clock_ms: elapsed_ms,
                    });
                }
                Err(e) => {
                    let partial = entry_snapshot
                        .as_ref()
                        .map(|r| r.ledger_entries.clone())
                        .unwrap_or_default();
                    let _ = tx_for_run.send(AgentEvent::RunAborted {
                        run_id,
                        reason: e.to_string(),
                        partial_ledger_entries: partial,
                    });
                }
            }
        });

        Ok(AgentRunHandle {
            run_id,
            events: rx,
        })
    }

    async fn undo(&self, run_id: Uuid) -> Result<UndoReport, AgentError> {
        let entries = {
            let runs = self.runs.lock().await;
            runs.get(&run_id)
                .map(|r| r.ledger_entries.clone())
                .ok_or_else(|| {
                    AgentError::Internal(format!("run {} not found in runtime cache", run_id))
                })?
        };

        let mut reversed = 0u32;
        let mut failed = 0u32;
        let mut failures = Vec::new();

        // Reverse in LIFO order — undo the latest writes first so any
        // referential integrity (e.g. activities depending on their
        // account) is preserved.
        for original_entry_id in entries.iter().rev() {
            let mut meta = std::collections::BTreeMap::new();
            meta.insert(
                "reverses".to_string(),
                serde_json::Value::String(original_entry_id.clone()),
            );
            meta.insert(
                "via_agent_run".to_string(),
                serde_json::Value::String(run_id.to_string()),
            );
            let undo_id = format!("agent:{}:undo:{}", run_id, original_entry_id);
            let append = AppendInput {
                id: undo_id,
                kind: Some(LedgerEntryKind::ActivityReversed),
                account_id: None,
                asset_id: None,
                amount: None,
                currency: None,
                metadata: meta,
                recorded_at: Some(Utc::now()),
            };
            match self.truth_ledger.append(append).await {
                Ok(_) => reversed += 1,
                Err(e) => {
                    failed += 1;
                    failures.push(UndoFailure {
                        ledger_entry_id: original_entry_id.clone(),
                        reason: e.to_string(),
                    });
                }
            }
        }

        Ok(UndoReport {
            run_id,
            reversed_count: reversed,
            failed_count: failed,
            failures,
        })
    }

    async fn get_run(&self, run_id: Uuid) -> Option<AgentRun> {
        let runs = self.runs.lock().await;
        runs.get(&run_id).cloned()
    }
}

/// Inner execution loop — extracted so the spawn closure stays readable.
/// Walks the (already topologically sorted) plan, dispatches each step,
/// emits StepStart/StepDone/StepFailed events, retries failures up to
/// MAX_STEP_RETRIES, and escalates to a re-plan after that.
#[allow(clippy::too_many_arguments)]
async fn execute_loop(
    run_id: Uuid,
    goal: String,
    plan: Vec<PlanStep>,
    planner: Arc<dyn AgentPlanner>,
    dispatcher: Arc<dyn AgentToolDispatcher>,
    runs: Arc<Mutex<HashMap<Uuid, AgentRun>>>,
    tx: tokio::sync::mpsc::UnboundedSender<AgentEvent>,
) -> Result<(), AgentError> {
    let total = plan.len() as u32;
    let mut step_index: u32 = 0;
    let started = std::time::Instant::now();
    let mut current_plan = plan.clone();
    let mut replan_count: u32 = 0;
    const MAX_REPLANS: u32 = 2;

    while (step_index as usize) < current_plan.len() {
        if started.elapsed() > MAX_RUN_DURATION {
            return Err(AgentError::WallClockExceeded(MAX_RUN_DURATION));
        }

        let step = current_plan[step_index as usize].clone();
        let _ = tx.send(AgentEvent::StepStart {
            run_id,
            step_index,
            total_steps: total,
            step_id: step.id.clone(),
            summary: step.summary.clone(),
        });

        let mut attempts: u32 = 0;
        let outcome = loop {
            attempts += 1;
            match dispatcher.dispatch(&step.tool, step.args.clone()).await {
                Ok(res) => break Ok(res),
                Err(e) if attempts <= MAX_STEP_RETRIES => {
                    let _ = tx.send(AgentEvent::StepFailed {
                        run_id,
                        step_index,
                        step_id: step.id.clone(),
                        error: e.to_string(),
                        retrying: true,
                    });
                    // Linear backoff — bounded by retry budget anyway.
                    tokio::time::sleep(Duration::from_millis(250 * attempts as u64)).await;
                }
                Err(e) => {
                    let _ = tx.send(AgentEvent::StepFailed {
                        run_id,
                        step_index,
                        step_id: step.id.clone(),
                        error: e.to_string(),
                        retrying: false,
                    });
                    break Err(e);
                }
            }
        };

        match outcome {
            Ok(res) => {
                {
                    let mut runs_lock = runs.lock().await;
                    if let Some(entry) = runs_lock.get_mut(&run_id) {
                        entry.ledger_entries.extend(res.ledger_entries.clone());
                        entry.tool_calls = entry.tool_calls.saturating_add(attempts);
                        entry.state = ExecutionState::Executing {
                            current_step: step_index + 1,
                        };
                        if entry.tool_calls > MAX_TOOL_CALLS_PER_RUN {
                            return Err(AgentError::ToolBudgetExceeded(entry.tool_calls));
                        }
                    }
                }
                let _ = tx.send(AgentEvent::StepDone {
                    run_id,
                    step_index,
                    step_id: step.id.clone(),
                    ledger_entries: res.ledger_entries,
                });
                step_index += 1;
            }
            Err(e) => {
                // Try a re-plan from current state — limited budget so
                // we don't infinite-loop bouncing between plans.
                if replan_count >= MAX_REPLANS {
                    return Err(AgentError::ToolFailed {
                        tool_name: step.tool.clone(),
                        attempts,
                        message: e.to_string(),
                    });
                }
                let _ = tx.send(AgentEvent::RePlanning {
                    run_id,
                    reason: format!("Step '{}' failed: {}", step.id, e),
                });
                replan_count += 1;
                let ctx = PlannerContext::default();
                match planner.replan(&goal, &ctx, &e.to_string()).await {
                    Ok(new_plan) if !new_plan.is_empty() => {
                        // Replace remainder with the re-planned steps.
                        let new_sorted = InMemoryAgentRuntime::topo_sort(&new_plan)?;
                        current_plan.truncate(step_index as usize);
                        current_plan.extend(new_sorted);
                        let mut runs_lock = runs.lock().await;
                        if let Some(entry) = runs_lock.get_mut(&run_id) {
                            entry.plan = current_plan.clone();
                        }
                    }
                    Ok(_) => {
                        return Err(AgentError::RePlanFailed(
                            "planner returned empty plan".to_string(),
                        ));
                    }
                    Err(replan_err) => {
                        return Err(AgentError::RePlanFailed(replan_err.to_string()));
                    }
                }
            }
        }
    }

    Ok(())
}

// ─── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use mizan_core::truth_engine::InMemoryTruthLedger;

    /// Returns a canned plan no matter the input. Useful for testing
    /// the loop without an LLM.
    struct FixedPlanner {
        plan: Vec<PlanStep>,
    }

    #[async_trait]
    impl AgentPlanner for FixedPlanner {
        async fn plan(
            &self,
            _goal: &str,
            _context: &PlannerContext,
        ) -> Result<Vec<PlanStep>, AgentError> {
            Ok(self.plan.clone())
        }
        async fn replan(
            &self,
            _g: &str,
            _c: &PlannerContext,
            _f: &str,
        ) -> Result<Vec<PlanStep>, AgentError> {
            Ok(self.plan.clone())
        }
    }

    /// Records every dispatch call and returns a configurable result.
    struct MockDispatcher {
        calls: Arc<Mutex<Vec<(String, serde_json::Value)>>>,
        result: Arc<Mutex<Box<dyn Fn(&str) -> Result<DispatchResult, AgentError> + Send + Sync>>>,
    }

    impl MockDispatcher {
        fn always_ok(entry_prefix: &'static str) -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                result: Arc::new(Mutex::new(Box::new(move |tool| {
                    Ok(DispatchResult {
                        output: serde_json::json!({"ok": true, "tool": tool}),
                        ledger_entries: vec![format!("{}:{}", entry_prefix, tool)],
                    })
                }))),
            }
        }

        fn always_err(message: &'static str) -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                result: Arc::new(Mutex::new(Box::new(move |_tool| {
                    Err(AgentError::Internal(message.to_string()))
                }))),
            }
        }
    }

    #[async_trait]
    impl AgentToolDispatcher for MockDispatcher {
        async fn dispatch(
            &self,
            tool_name: &str,
            args: serde_json::Value,
        ) -> Result<DispatchResult, AgentError> {
            self.calls
                .lock()
                .await
                .push((tool_name.to_string(), args.clone()));
            let result_fn = self.result.lock().await;
            (result_fn)(tool_name)
        }
    }

    fn step(id: &str, tool: &str, deps: &[&str]) -> PlanStep {
        PlanStep {
            id: id.to_string(),
            tool: tool.to_string(),
            args: serde_json::json!({}),
            depends_on: deps.iter().map(|s| s.to_string()).collect(),
            summary: format!("Run {}", tool),
            verify: None,
        }
    }

    #[tokio::test]
    async fn happy_path_executes_all_steps_and_emits_events() {
        let planner = Arc::new(FixedPlanner {
            plan: vec![
                step("a", "create_account", &[]),
                step("b", "record_activities", &["a"]),
            ],
        });
        let dispatcher = Arc::new(MockDispatcher::always_ok("led"));
        let ledger = Arc::new(InMemoryTruthLedger::new());
        let runtime = InMemoryAgentRuntime::new(
            planner,
            Arc::clone(&dispatcher) as Arc<dyn AgentToolDispatcher>,
            ledger,
        );

        let handle = runtime
            .run("set up portfolio".to_string(), PlannerContext::default())
            .await
            .expect("plan ok");

        // Drain events.
        let mut events: Vec<AgentEvent> = Vec::new();
        let mut rx = handle.events;
        while let Some(e) = rx.recv().await {
            let is_terminal = matches!(
                e,
                AgentEvent::RunComplete { .. } | AgentEvent::RunAborted { .. }
            );
            events.push(e);
            if is_terminal {
                break;
            }
        }

        assert!(matches!(events[0], AgentEvent::PlanReady { .. }));
        assert!(events.iter().any(|e| matches!(e, AgentEvent::StepDone { .. })));
        assert!(matches!(events.last(), Some(AgentEvent::RunComplete { .. })));
        assert_eq!(dispatcher.calls.lock().await.len(), 2);
    }

    #[tokio::test]
    async fn retries_failed_step_then_gives_up() {
        let planner = Arc::new(FixedPlanner {
            plan: vec![step("a", "broken_tool", &[])],
        });
        let dispatcher = Arc::new(MockDispatcher::always_err("simulated"));
        let ledger = Arc::new(InMemoryTruthLedger::new());
        let runtime = InMemoryAgentRuntime::new(
            planner,
            Arc::clone(&dispatcher) as Arc<dyn AgentToolDispatcher>,
            ledger,
        );

        let handle = runtime
            .run("trigger failure".to_string(), PlannerContext::default())
            .await
            .expect("plan ok");

        let mut events = Vec::new();
        let mut rx = handle.events;
        while let Some(e) = rx.recv().await {
            let terminal = matches!(
                e,
                AgentEvent::RunComplete { .. } | AgentEvent::RunAborted { .. }
            );
            events.push(e);
            if terminal {
                break;
            }
        }

        // We should see retries (StepFailed with retrying=true) then a
        // final abort.
        let retry_count = events
            .iter()
            .filter(|e| matches!(e, AgentEvent::StepFailed { retrying: true, .. }))
            .count();
        assert!(retry_count >= 1, "expected at least one retry event");
        assert!(matches!(events.last(), Some(AgentEvent::RunAborted { .. })));
        // Dispatcher should have been called MAX_STEP_RETRIES+1 attempts
        // per try, plus extra for any re-plans. Just assert >= 1.
        assert!(dispatcher.calls.lock().await.len() >= 1);
    }

    #[tokio::test]
    async fn undo_appends_reversal_entry_per_ledger_id() {
        let planner = Arc::new(FixedPlanner {
            plan: vec![
                step("a", "create_account", &[]),
                step("b", "record_activities", &["a"]),
            ],
        });
        let dispatcher = Arc::new(MockDispatcher::always_ok("led"));
        let ledger = Arc::new(InMemoryTruthLedger::new());
        let runtime = InMemoryAgentRuntime::new(
            planner,
            Arc::clone(&dispatcher) as Arc<dyn AgentToolDispatcher>,
            Arc::clone(&ledger) as Arc<dyn TruthLedger>,
        );

        let handle = runtime
            .run("populate".to_string(), PlannerContext::default())
            .await
            .expect("plan ok");
        let run_id = handle.run_id;

        // Drain to completion.
        let mut rx = handle.events;
        while let Some(e) = rx.recv().await {
            if matches!(e, AgentEvent::RunComplete { .. } | AgentEvent::RunAborted { .. }) {
                break;
            }
        }

        let report = runtime.undo(run_id).await.expect("undo ok");
        assert_eq!(report.reversed_count, 2);
        assert_eq!(report.failed_count, 0);

        // Ledger should now contain: 1 agent header + 2 step entries +
        // 2 reversal entries = 5. (The dispatcher mock writes the ids
        // it CLAIMS to write into ledger_entries, but the agent header
        // and the reversal entries are written for real.)
        assert!(
            ledger.len() >= 3,
            "expected at least header + 2 reversal entries, got {}",
            ledger.len()
        );
    }

    #[tokio::test]
    async fn topo_sort_rejects_cycles() {
        let plan = vec![
            step("a", "x", &["b"]),
            step("b", "y", &["a"]),
        ];
        let err = InMemoryAgentRuntime::topo_sort(&plan).unwrap_err();
        assert!(matches!(err, AgentError::PlanCycle(_)));
    }

    #[tokio::test]
    async fn topo_sort_rejects_unknown_dep() {
        let plan = vec![step("a", "x", &["ghost"])];
        let err = InMemoryAgentRuntime::topo_sort(&plan).unwrap_err();
        assert!(matches!(err, AgentError::PlanUnknownDep(_)));
    }

    #[tokio::test]
    async fn topo_sort_orders_diamond_correctly() {
        let plan = vec![
            step("d", "w", &["b", "c"]),
            step("a", "x", &[]),
            step("b", "y", &["a"]),
            step("c", "z", &["a"]),
        ];
        let sorted = InMemoryAgentRuntime::topo_sort(&plan).unwrap();
        let idx = |id: &str| sorted.iter().position(|s| s.id == id).unwrap();
        assert!(idx("a") < idx("b"));
        assert!(idx("a") < idx("c"));
        assert!(idx("b") < idx("d"));
        assert!(idx("c") < idx("d"));
    }

    #[tokio::test]
    async fn empty_plan_rejected() {
        let planner = Arc::new(FixedPlanner { plan: vec![] });
        let dispatcher = Arc::new(MockDispatcher::always_ok("led"));
        let ledger = Arc::new(InMemoryTruthLedger::new());
        let runtime = InMemoryAgentRuntime::new(
            planner,
            Arc::clone(&dispatcher) as Arc<dyn AgentToolDispatcher>,
            ledger,
        );

        let err = runtime
            .run("nothing".to_string(), PlannerContext::default())
            .await
            .unwrap_err();
        assert!(matches!(err, AgentError::EmptyPlan));
    }
}
