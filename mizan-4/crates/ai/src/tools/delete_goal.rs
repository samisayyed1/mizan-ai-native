//! Delete Goal tool — propose deletion of an existing goal.
//!
//! Companion to `create_goal`. When the user says "delete my retirement
//! goal" or "remove the goal I made by mistake", the AI calls this with
//! a goal reference (id or title fragment). Returns a draft preview with
//! the target goal + progress summary so the user knows what they're
//! about to lose before clicking Confirm in the tool-call card. The tool
//! itself never writes — the actual delete runs on user confirmation
//! through the existing goal-mutations hook.

use log::debug;
use rig::{completion::ToolDefinition, tool::Tool};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::env::AiEnvironment;
use crate::error::AiError;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteGoalArgs {
    /// Either the goal id or a title match (exact or substring).
    pub goal_ref: String,
    /// Optional free-form reason the user gave — surfaced on the
    /// confirm card so the user can sanity-check the AI's intent.
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteGoalOutput {
    pub target: GoalSnapshot,
    pub impact: DeletionImpact,
    pub reason: Option<String>,
    pub validation: ValidationResult,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalSnapshot {
    pub id: String,
    pub title: String,
    pub goal_type: String,
    pub target_amount: Option<f64>,
    pub currency: Option<String>,
    pub status_lifecycle: String,
    pub progress_percent: Option<f64>,
    pub current_value: Option<f64>,
    pub target_date: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletionImpact {
    /// True when this goal had funding rules attached — surfacing this
    /// makes the user aware that the rules go away too.
    pub had_funding_rules: bool,
    /// True when this goal had an attached plan (retirement plan etc.).
    pub had_plan: bool,
    /// True when the goal is in an active lifecycle state — deleting an
    /// active goal is more destructive than tossing a draft.
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    pub resolved: bool,
    pub candidates: Vec<GoalSnapshot>,
    pub warnings: Vec<String>,
    pub is_valid: bool,
}

enum Resolution {
    Single(Box<mizan_core::goals::Goal>),
    Ambiguous(Vec<mizan_core::goals::Goal>),
    NotFound,
    Missing,
}

fn snapshot(g: &mizan_core::goals::Goal) -> GoalSnapshot {
    let progress = match (g.summary_current_value, g.target_amount) {
        (Some(cur), Some(target)) if target > 0.0 => Some((cur / target) * 100.0),
        _ => g.summary_progress,
    };
    GoalSnapshot {
        id: g.id.clone(),
        title: g.title.clone(),
        goal_type: g.goal_type.clone(),
        target_amount: g.target_amount,
        currency: g.currency.clone(),
        status_lifecycle: g.status_lifecycle.clone(),
        progress_percent: progress,
        current_value: g.summary_current_value,
        target_date: g.target_date.clone(),
    }
}

fn resolve(goals: &[mizan_core::goals::Goal], reference: &str) -> Resolution {
    let r = reference.trim();
    if r.is_empty() {
        return Resolution::Missing;
    }
    if let Some(g) = goals.iter().find(|g| g.id == r) {
        return Resolution::Single(Box::new(g.clone()));
    }
    let lower = r.to_lowercase();
    let exact: Vec<_> = goals
        .iter()
        .filter(|g| g.title.to_lowercase() == lower)
        .cloned()
        .collect();
    if exact.len() == 1 {
        return Resolution::Single(Box::new(exact.into_iter().next().unwrap()));
    }
    if exact.len() > 1 {
        return Resolution::Ambiguous(exact);
    }
    let substring: Vec<_> = goals
        .iter()
        .filter(|g| g.title.to_lowercase().contains(&lower))
        .cloned()
        .collect();
    match substring.len() {
        0 => Resolution::NotFound,
        1 => Resolution::Single(Box::new(substring.into_iter().next().unwrap())),
        _ => Resolution::Ambiguous(substring),
    }
}

pub struct DeleteGoalTool<E: AiEnvironment> {
    env: Arc<E>,
}

impl<E: AiEnvironment> DeleteGoalTool<E> {
    pub fn new(env: Arc<E>) -> Self {
        Self { env }
    }

    pub(crate) async fn build_output(
        &self,
        args: DeleteGoalArgs,
    ) -> Result<DeleteGoalOutput, AiError> {
        debug!(
            "delete_goal called: ref={:?}, reason={:?}",
            args.goal_ref, args.reason
        );

        let goal_service = self.env.goal_service();
        let goals = goal_service
            .get_goals()
            .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;

        let resolution = resolve(&goals, &args.goal_ref);

        match resolution {
            Resolution::Single(boxed) => {
                let goal = *boxed;
                let had_funding_rules = goal_service
                    .get_goal_funding(&goal.id)
                    .map(|rules| !rules.is_empty())
                    .unwrap_or(false);
                let had_plan = goal_service
                    .get_goal_plan(&goal.id)
                    .map(|opt| opt.is_some())
                    .unwrap_or(false);

                let is_active = matches!(
                    goal.status_lifecycle.to_uppercase().as_str(),
                    "ACTIVE" | "IN_PROGRESS" | "ON_TRACK"
                );

                let mut warnings = Vec::new();
                if is_active {
                    warnings.push(
                        "This goal is active — consider pausing it instead if you're not \
                         sure you want to throw away the progress permanently."
                            .to_string(),
                    );
                }
                if had_funding_rules {
                    warnings.push(
                        "Funding rules attached to this goal will be removed too.".to_string(),
                    );
                }
                if had_plan {
                    warnings.push(
                        "An attached plan (retirement / savings simulation) will be lost — \
                         the inputs you entered there won't survive the delete."
                            .to_string(),
                    );
                }

                Ok(DeleteGoalOutput {
                    target: snapshot(&goal),
                    impact: DeletionImpact {
                        had_funding_rules,
                        had_plan,
                        is_active,
                    },
                    reason: args.reason,
                    validation: ValidationResult {
                        resolved: true,
                        candidates: Vec::new(),
                        warnings,
                        is_valid: true,
                    },
                })
            }
            Resolution::Ambiguous(candidates) => Ok(DeleteGoalOutput {
                target: GoalSnapshot::default(),
                impact: DeletionImpact::default(),
                reason: args.reason,
                validation: ValidationResult {
                    resolved: false,
                    candidates: candidates.iter().map(snapshot).collect(),
                    warnings: vec![format!(
                        "\"{}\" matched {} goals. Ask the user which one they mean by title \
                         + target date before retrying.",
                        args.goal_ref,
                        candidates.len()
                    )],
                    is_valid: false,
                },
            }),
            Resolution::NotFound => Ok(DeleteGoalOutput {
                target: GoalSnapshot::default(),
                impact: DeletionImpact::default(),
                reason: args.reason,
                validation: ValidationResult {
                    resolved: false,
                    candidates: Vec::new(),
                    warnings: vec![format!(
                        "No goal matching \"{}\" found. Use get_goals to list active goals first.",
                        args.goal_ref
                    )],
                    is_valid: false,
                },
            }),
            Resolution::Missing => Ok(DeleteGoalOutput {
                target: GoalSnapshot::default(),
                impact: DeletionImpact::default(),
                reason: args.reason,
                validation: ValidationResult {
                    resolved: false,
                    candidates: Vec::new(),
                    warnings: vec!["goalRef is required — give the goal title or id.".to_string()],
                    is_valid: false,
                },
            }),
        }
    }
}

impl<E: AiEnvironment + 'static> Tool for DeleteGoalTool<E> {
    const NAME: &'static str = "delete_goal";

    type Error = AiError;
    type Args = DeleteGoalArgs;
    type Output = DeleteGoalOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description:
                "Propose deletion of an existing goal. Use when the user wants to \
                 remove a goal they no longer care about (mistyped goal, plan changed, \
                 already-achieved goal they don't need to track). Returns a draft \
                 with the target goal + progress summary + warnings — the user clicks \
                 Confirm in an inline card to actually delete. Does not write directly. \
                 If the user is uncertain whether to delete vs pause an active goal, \
                 ASK — delete is irreversible."
                    .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "goalRef": {
                        "type": "string",
                        "description": "Goal id or title match (exact or substring). \
                         E.g. 'House down payment', 'retirement', or the canonical id from \
                         get_goals."
                    },
                    "reason": {
                        "type": "string",
                        "description": "Optional reason from the user — surfaced on the confirm \
                         card. E.g. 'I already bought the house', 'I changed my mind about that goal'."
                    }
                },
                "required": ["goalRef"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        self.build_output(args).await
    }
}
