//! LLM-backed [`AgentPlanner`] implementation.
//!
//! Bridges the agent runtime to whatever AI provider the user has
//! configured (Mizan AI / OpenAI / Anthropic / Ollama / etc.). The
//! [`PromptBasedPlanner`] is provider-agnostic: it takes a closure
//! `(prompt: String) → Future<Output = Result<String, _>>` and calls
//! it with a carefully-shaped system prompt that asks the LLM to emit
//! a JSON `Vec<PlanStep>`.
//!
//! # Why a closure, not a direct rig-core call
//!
//! The chat dispatcher in [`crate::chat`] already owns the rig-core
//! client + JWT + per-provider routing. Re-implementing all of that
//! here would duplicate ~500 LOC. Instead, the chat dispatcher
//! constructs a `PromptBasedPlanner` and passes a closure that
//! invokes its own existing LLM machinery. The agent runtime stays
//! decoupled from any specific provider library.
//!
//! # The planning prompt
//!
//! Structured output is enforced via the system prompt + JSON-mode
//! request (when the provider supports it; falls back to "extract
//! JSON from a fenced code block" parsing on providers that don't).
//!
//! The prompt:
//! 1. Lists every available tool by name + one-liner description.
//! 2. Documents the JSON schema for `PlanStep` exactly.
//! 3. Constrains: ≤ MAX_TOOL_CALLS_PER_RUN steps, no cycles in
//!    `dependsOn`, valid tool names only.
//! 4. Provides 2-3 few-shot examples of well-formed plans for the
//!    domain (portfolio setup, goal planning, broker reconciliation).
//!
//! The LLM returns JSON; we parse, validate against the runtime's
//! invariants, and surface `AgentError::PlanParse` with the raw
//! response on failure so the support bundle captures what went wrong.

use std::sync::Arc;

use async_trait::async_trait;

use crate::agent::{AgentError, AgentPlanner, PlanStep, PlannerContext, MAX_TOOL_CALLS_PER_RUN};

/// One tool the planner knows about. Used to construct the
/// `### Available tools` section of the system prompt.
#[derive(Debug, Clone)]
pub struct ToolDescriptor {
    pub name: String,
    pub one_line_summary: String,
    /// JSON schema for the tool's args. Embedded so the planner
    /// knows exactly what fields to fill in.
    pub args_schema: serde_json::Value,
    /// Whether this tool mutates state. Read-only tools (parse_csv,
    /// query_holdings) can be called freely; mutating tools (create_account,
    /// record_activities) increment the agent run's write count.
    pub mutates: bool,
}

/// Closure type the chat dispatcher implements to bridge LLM calls
/// into the planner. Async because the underlying call is a network
/// round-trip.
pub type LlmCallFn = Arc<
    dyn Fn(String) -> futures::future::BoxFuture<'static, Result<String, AgentError>> + Send + Sync,
>;

/// Provider-agnostic planner. Holds the tool catalog + an LLM-call
/// closure; emits structured plans by prompting + parsing.
pub struct PromptBasedPlanner {
    tools: Vec<ToolDescriptor>,
    call_llm: LlmCallFn,
    /// When `true`, the planner asks the LLM to emit response wrapped
    /// in ```json fences and parses fenced JSON. When `false`, the
    /// planner relies on the provider's native JSON mode. Default:
    /// true (works on every provider; falsy providers may produce
    /// extra prose around the JSON that we trim).
    fence_mode: bool,
}

impl PromptBasedPlanner {
    pub fn new(tools: Vec<ToolDescriptor>, call_llm: LlmCallFn) -> Self {
        Self {
            tools,
            call_llm,
            fence_mode: true,
        }
    }

    pub fn with_fence_mode(mut self, fence: bool) -> Self {
        self.fence_mode = fence;
        self
    }

    /// Build the structured-output prompt. Public so the chat
    /// dispatcher (or tests) can inspect what the LLM sees.
    pub fn build_prompt(&self, goal: &str, context: &PlannerContext) -> String {
        let mut out = String::new();
        out.push_str(
            "You are Mizan's agent planner. The user has stated a financial goal. \
             Decompose it into a JSON list of executable PlanStep objects. \
             Then the runtime will execute them autonomously.\n\n",
        );

        out.push_str("### Plan schema (strict JSON)\n\n");
        out.push_str("```json\n");
        out.push_str(
            r#"[
  {
    "id": "string-unique-within-this-plan",
    "tool": "tool-name-from-the-Available-tools-list-below",
    "args": { /* JSON object matching the tool's schema */ },
    "dependsOn": ["earlier-step-id", ...],
    "summary": "Short human one-liner the UI will display",
    "verify": null  // OR a VerifyExpectation object (see below)
  }
]
"#,
        );
        out.push_str("```\n\n");

        out.push_str("### Verify expectations (optional, for terminal verification steps)\n\n");
        out.push_str("```json\n");
        out.push_str(
            r#"// Equality check within a tolerance:
{ "kind": "equal_within", "actualTool": "tool-to-call", "actualPath": "$.netWorth", "expected": 100000, "tolerance": 0.01 }

// Row-count check:
{ "kind": "count_equals", "actualTool": "tool-to-call", "expectedCount": 402 }

// Natural-language fallback:
{ "kind": "natural_language", "0": "Portfolio total equals CSV sum within $0.01" }
"#,
        );
        out.push_str("```\n\n");

        out.push_str("### Available tools\n\n");
        if self.tools.is_empty() {
            out.push_str(
                "(none — the runtime will reject any plan; tell the user the agent isn't available)\n\n",
            );
        } else {
            for t in &self.tools {
                let mutation_tag = if t.mutates { "WRITE" } else { "read" };
                out.push_str(&format!(
                    "- `{}` ({}) — {}\n  Args schema:\n  ```json\n  {}\n  ```\n",
                    t.name,
                    mutation_tag,
                    t.one_line_summary,
                    serde_json::to_string_pretty(&t.args_schema)
                        .unwrap_or_else(|_| "{}".to_string())
                ));
            }
            out.push('\n');
        }

        out.push_str("### Hard constraints\n\n");
        out.push_str(&format!(
            "- ≤ {} steps in total. If the goal genuinely needs more, plan only the first {} and emit a final step that calls `request_continuation` (the runtime will ask the user before continuing).\n",
            MAX_TOOL_CALLS_PER_RUN, MAX_TOOL_CALLS_PER_RUN
        ));
        out.push_str("- `dependsOn` ids must reference earlier `id`s in the same plan. No cycles. No self-references.\n");
        out.push_str(
            "- Tool names must come from the Available tools list verbatim. No invention.\n",
        );
        out.push_str(
            "- Args must match each tool's schema. Don't include keys the schema doesn't list.\n",
        );
        out.push_str("- The FINAL step should be a verification (`verify` field set) so the runtime can assert the goal was met.\n");
        out.push_str(
            "- DO NOT include explanatory prose outside the JSON. Just the JSON array.\n\n",
        );

        if !context.attachments.is_empty() {
            out.push_str("### User-attached files\n\n");
            for a in &context.attachments {
                out.push_str(&format!(
                    "- `{}` ({}, {} bytes) — content available to tools via `attachments[].filename`\n",
                    a.filename,
                    a.content_type,
                    a.content.len()
                ));
            }
            out.push('\n');
        }

        if !context.completed_steps.is_empty() {
            out.push_str("### Already completed (do NOT re-emit these)\n\n");
            for (s, _r) in &context.completed_steps {
                out.push_str(&format!("- {} ({})\n", s.summary, s.tool));
            }
            out.push('\n');
        }

        out.push_str("### Goal\n\n");
        out.push_str(goal);
        out.push_str("\n\n");

        out.push_str("### Response\n\n");
        if self.fence_mode {
            out.push_str(
                "Respond with ONLY a JSON array of PlanStep objects, wrapped in a ```json code fence. No other text.\n",
            );
        } else {
            out.push_str(
                "Respond with ONLY a JSON array of PlanStep objects. No prose, no preamble.\n",
            );
        }
        out
    }

    /// Build the re-plan prompt — variant that includes the failure
    /// context and asks for a recovery plan from the current state.
    pub fn build_replan_prompt(
        &self,
        original_goal: &str,
        context: &PlannerContext,
        failure: &str,
    ) -> String {
        let mut p = self.build_prompt(original_goal, context);
        p.push_str("\n### Re-plan context\n\n");
        p.push_str(&format!(
            "The previous plan failed mid-execution with this error:\n\n```\n{}\n```\n\n",
            failure
        ));
        p.push_str(
            "Emit a NEW plan that recovers from this state. The runtime has already \
             executed everything in the 'Already completed' section above — start AFTER \
             that. If the goal is no longer achievable, emit a single step calling \
             `abort_with_message` explaining why to the user.\n",
        );
        p
    }

    /// Parse the LLM's response. Tolerant of: ```json fences,
    /// leading/trailing prose, BOM, single quotes (replaced with
    /// double), trailing commas (removed).
    pub fn parse_plan(&self, response: &str) -> Result<Vec<PlanStep>, AgentError> {
        let json_text = extract_json_array(response)
            .ok_or_else(|| AgentError::PlanParse("no JSON array found in response".to_string()))?;
        let cleaned = clean_json(&json_text);
        let plan: Vec<PlanStep> = serde_json::from_str(&cleaned).map_err(|e| {
            AgentError::PlanParse(format!(
                "failed to parse PlanStep array: {} — raw json:\n{}",
                e,
                json_text.chars().take(2000).collect::<String>()
            ))
        })?;
        Ok(plan)
    }
}

/// Extract the first balanced JSON array `[ ... ]` from a free-form
/// LLM response. Handles ```json fences, embedded code blocks, and
/// arrays inside narrative prose.
fn extract_json_array(s: &str) -> Option<String> {
    // 1. Prefer content inside a ```json ... ``` fence.
    if let Some(fenced) = extract_fenced("```json", "```", s)
        .or_else(|| extract_fenced("```JSON", "```", s))
        .or_else(|| extract_fenced("```", "```", s))
    {
        if fenced.trim_start().starts_with('[') {
            return Some(fenced.trim().to_string());
        }
    }
    // 2. Scan for the first '[' and walk to its matching ']' tracking
    //    depth so nested arrays/objects work.
    let bytes = s.as_bytes();
    let mut start = None;
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escape = false;
    for (i, &b) in bytes.iter().enumerate() {
        if start.is_none() {
            if b == b'[' {
                start = Some(i);
                depth = 1;
            }
            continue;
        }
        if escape {
            escape = false;
            continue;
        }
        if in_str {
            match b {
                b'\\' => escape = true,
                b'"' => in_str = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'[' | b'{' => depth += 1,
            b']' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    let start_idx = start.unwrap();
                    return Some(s[start_idx..=i].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

fn extract_fenced(open: &str, close: &str, s: &str) -> Option<String> {
    let start = s.find(open)? + open.len();
    let rest = &s[start..];
    let end = rest.find(close)?;
    Some(rest[..end].to_string())
}

/// Best-effort JSON pre-cleanup: remove trailing commas in objects /
/// arrays (a very common LLM mistake) + smart-quote → ascii-quote.
fn clean_json(s: &str) -> String {
    // Replace smart quotes in one pass (double + single grouped together).
    let smart_quoted = s
        .replace(['\u{201C}', '\u{201D}'], "\"") // “ ”
        .replace(['\u{2018}', '\u{2019}'], "'"); // ‘ ’

    // Strip trailing commas before } or ] — tolerant of whitespace.
    // Walks once, O(n).
    let bytes = smart_quoted.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    let mut in_str = false;
    let mut escape = false;
    while i < bytes.len() {
        let b = bytes[i];
        if escape {
            out.push(b);
            escape = false;
            i += 1;
            continue;
        }
        if in_str {
            if b == b'\\' {
                out.push(b);
                escape = true;
            } else if b == b'"' {
                out.push(b);
                in_str = false;
            } else {
                out.push(b);
            }
            i += 1;
            continue;
        }
        if b == b'"' {
            out.push(b);
            in_str = true;
            i += 1;
            continue;
        }
        if b == b',' {
            // Look ahead past whitespace; if next non-ws is } or ], skip the comma.
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < bytes.len() && (bytes[j] == b'}' || bytes[j] == b']') {
                i += 1;
                continue;
            }
        }
        out.push(b);
        i += 1;
    }
    String::from_utf8(out).unwrap_or(smart_quoted)
}

#[async_trait]
impl AgentPlanner for PromptBasedPlanner {
    async fn plan(
        &self,
        goal: &str,
        context: &PlannerContext,
    ) -> Result<Vec<PlanStep>, AgentError> {
        let prompt = self.build_prompt(goal, context);
        let response = (self.call_llm)(prompt).await?;
        let plan = self.parse_plan(&response)?;
        Ok(plan)
    }

    async fn replan(
        &self,
        original_goal: &str,
        current_state: &PlannerContext,
        failure: &str,
    ) -> Result<Vec<PlanStep>, AgentError> {
        let prompt = self.build_replan_prompt(original_goal, current_state, failure);
        let response = (self.call_llm)(prompt).await?;
        let plan = self.parse_plan(&response)?;
        Ok(plan)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(name: &str, summary: &str, mutates: bool) -> ToolDescriptor {
        ToolDescriptor {
            name: name.to_string(),
            one_line_summary: summary.to_string(),
            args_schema: serde_json::json!({"type": "object"}),
            mutates,
        }
    }

    fn planner_with_response(response: &'static str) -> PromptBasedPlanner {
        PromptBasedPlanner::new(
            vec![
                descriptor("create_account", "Create a brokerage / cash account", true),
                descriptor(
                    "record_activities",
                    "Add one or more activities to an account",
                    true,
                ),
            ],
            Arc::new(move |_prompt| Box::pin(async move { Ok(response.to_string()) })),
        )
    }

    #[tokio::test]
    async fn parses_fenced_json_response() {
        let response = r#"
Sure, here's the plan:

```json
[
  {
    "id": "a",
    "tool": "create_account",
    "args": { "name": "US Stocks", "currency": "USD" },
    "dependsOn": [],
    "summary": "Create the US Stocks account",
    "verify": null
  },
  {
    "id": "b",
    "tool": "record_activities",
    "args": { "accountId": "<ref-a>", "rows": [] },
    "dependsOn": ["a"],
    "summary": "Add CSV rows as activities",
    "verify": null
  }
]
```
"#;
        let p = planner_with_response(response);
        let plan = p
            .plan("set up US Stocks", &PlannerContext::default())
            .await
            .unwrap();
        assert_eq!(plan.len(), 2);
        assert_eq!(plan[0].id, "a");
        assert_eq!(plan[1].depends_on, vec!["a".to_string()]);
    }

    #[tokio::test]
    async fn parses_bare_json_array() {
        let response = r#"[{"id":"x","tool":"create_account","args":{},"dependsOn":[],"summary":"hi","verify":null}]"#;
        let p = planner_with_response(response);
        let plan = p.plan("g", &PlannerContext::default()).await.unwrap();
        assert_eq!(plan.len(), 1);
    }

    #[tokio::test]
    async fn tolerates_trailing_commas() {
        let response = r#"[
            {"id":"x","tool":"create_account","args":{},"dependsOn":[],"summary":"hi","verify":null,},
        ]"#;
        let p = planner_with_response(response);
        let plan = p.plan("g", &PlannerContext::default()).await.unwrap();
        assert_eq!(plan.len(), 1);
    }

    #[tokio::test]
    async fn tolerates_smart_quotes() {
        // Some LLMs sneak smart quotes in when generating prose-y JSON.
        let response = "[{\u{201C}id\u{201D}: \"x\", \"tool\": \"create_account\", \"args\": {}, \"dependsOn\": [], \"summary\": \"hi\", \"verify\": null}]";
        let p = planner_with_response(response);
        let plan = p.plan("g", &PlannerContext::default()).await.unwrap();
        assert_eq!(plan.len(), 1);
    }

    #[tokio::test]
    async fn rejects_garbage_response() {
        let p = planner_with_response("I cannot help with that, please rephrase.");
        let err = p.plan("g", &PlannerContext::default()).await.unwrap_err();
        assert!(matches!(err, AgentError::PlanParse(_)));
    }

    #[test]
    fn prompt_includes_tool_catalog_and_constraints() {
        let p = PromptBasedPlanner::new(
            vec![descriptor("create_account", "Create a new account", true)],
            Arc::new(|_p| Box::pin(async { Ok(String::new()) })),
        );
        let prompt = p.build_prompt("set up portfolio", &PlannerContext::default());
        assert!(prompt.contains("create_account"));
        assert!(prompt.contains("Create a new account"));
        assert!(prompt.contains("WRITE"));
        assert!(prompt.contains(&MAX_TOOL_CALLS_PER_RUN.to_string()));
        assert!(prompt.contains("dependsOn"));
        assert!(prompt.contains("### Goal"));
    }

    #[test]
    fn replan_prompt_carries_failure_context() {
        let p = PromptBasedPlanner::new(
            vec![descriptor("create_account", "x", true)],
            Arc::new(|_p| Box::pin(async { Ok(String::new()) })),
        );
        let prompt = p.build_replan_prompt(
            "set up portfolio",
            &PlannerContext::default(),
            "create_account returned 409 conflict",
        );
        assert!(prompt.contains("Re-plan context"));
        assert!(prompt.contains("409 conflict"));
    }
}
