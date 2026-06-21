//! Update Goal tool — edit an existing goal from natural language.
//!
//! Companion to `create_goal` / `delete_goal`. When the user says
//! "bump the retirement goal to 2M" or "push the house goal to 2030",
//! the AI calls this with a goal reference (id or title fragment) plus
//! the fields it wants to change. Returns a DRAFT preview the user
//! confirms — the tool itself never writes; the actual mutation runs
//! through `goal_service.update_goal` on confirmation via the existing
//! goal-mutations hook on the frontend.

use log::debug;
use rig::{completion::ToolDefinition, tool::Tool};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::env::AiEnvironment;
use crate::error::AiError;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateGoalArgs {
    /// Either the goal id or a title match (exact or substring).
    pub goal_ref: String,
    /// New title (omit to keep current).
    pub title: Option<String>,
    /// New target amount in goal currency (omit to keep current).
    pub target_amount: Option<f64>,
    /// New target date ISO 8601 (YYYY-MM-DD; omit to keep current).
    pub target_date: Option<String>,
    /// New priority 1-3 (1 = highest; omit to keep current).
    pub priority: Option<i32>,
    /// New lifecycle status (ACTIVE | PAUSED | DRAFT | ACHIEVED | ARCHIVED).
    pub status_lifecycle: Option<String>,
    /// New description (free-form; pass empty string to clear).
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateGoalOutput {
    pub draft: GoalUpdateDraft,
    pub current: GoalSnapshot,
    pub diff: Vec<FieldDiff>,
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
    pub priority: i32,
    pub target_date: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalUpdateDraft {
    pub id: String,
    pub title: String,
    pub target_amount: Option<f64>,
    pub status_lifecycle: String,
    pub priority: i32,
    pub target_date: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldDiff {
    pub field: String,
    pub old: Option<String>,
    pub new: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    pub resolved: bool,
    pub candidates: Vec<GoalSnapshot>,
    pub warnings: Vec<String>,
    pub is_valid: bool,
    pub has_changes: bool,
}

const VALID_LIFECYCLE: &[&str] = &["ACTIVE", "PAUSED", "DRAFT", "ACHIEVED", "ARCHIVED"];

fn snapshot(g: &mizan_core::goals::Goal) -> GoalSnapshot {
    GoalSnapshot {
        id: g.id.clone(),
        title: g.title.clone(),
        goal_type: g.goal_type.clone(),
        target_amount: g.target_amount,
        currency: g.currency.clone(),
        status_lifecycle: g.status_lifecycle.clone(),
        priority: g.priority,
        target_date: g.target_date.clone(),
        description: g.description.clone(),
    }
}

enum Resolution {
    Single(Box<mizan_core::goals::Goal>),
    Ambiguous(Vec<mizan_core::goals::Goal>),
    NotFound,
    Missing,
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

fn validate_date(s: &str) -> bool {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok()
}

pub struct UpdateGoalTool<E: AiEnvironment> {
    env: Arc<E>,
}

impl<E: AiEnvironment> UpdateGoalTool<E> {
    pub fn new(env: Arc<E>) -> Self {
        Self { env }
    }

    pub(crate) async fn build_output(
        &self,
        args: UpdateGoalArgs,
    ) -> Result<UpdateGoalOutput, AiError> {
        debug!("update_goal called: ref={:?}", args.goal_ref);

        let goal_service = self.env.goal_service();
        let goals = goal_service
            .get_goals()
            .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;

        let mut warnings: Vec<String> = Vec::new();

        let resolution = resolve(&goals, &args.goal_ref);

        match resolution {
            Resolution::Single(boxed) => {
                let current = *boxed;
                let mut draft = GoalUpdateDraft {
                    id: current.id.clone(),
                    title: current.title.clone(),
                    target_amount: current.target_amount,
                    status_lifecycle: current.status_lifecycle.clone(),
                    priority: current.priority,
                    target_date: current.target_date.clone(),
                    description: current.description.clone(),
                };
                let mut diff: Vec<FieldDiff> = Vec::new();

                if let Some(t) = args.title.as_ref() {
                    let trimmed = t.trim();
                    if trimmed.is_empty() {
                        warnings.push("Title can't be cleared — provide a non-empty title or omit the field.".to_string());
                    } else if trimmed != current.title {
                        diff.push(FieldDiff {
                            field: "title".to_string(),
                            old: Some(current.title.clone()),
                            new: Some(trimmed.to_string()),
                        });
                        draft.title = trimmed.to_string();
                    }
                }

                if let Some(amt) = args.target_amount {
                    if !amt.is_finite() || amt < 0.0 {
                        warnings.push(format!(
                            "Target amount {} is invalid — drop it or send a positive number.",
                            amt
                        ));
                    } else if Some(amt) != current.target_amount {
                        diff.push(FieldDiff {
                            field: "targetAmount".to_string(),
                            old: current.target_amount.map(|v| v.to_string()),
                            new: Some(amt.to_string()),
                        });
                        draft.target_amount = Some(amt);
                    }
                }

                if let Some(td) = args.target_date.as_ref() {
                    let trimmed = td.trim();
                    if trimmed.is_empty() {
                        if current.target_date.is_some() {
                            diff.push(FieldDiff {
                                field: "targetDate".to_string(),
                                old: current.target_date.clone(),
                                new: None,
                            });
                            draft.target_date = None;
                        }
                    } else if !validate_date(trimmed) {
                        warnings.push(format!(
                            "Target date '{}' is not ISO 8601 (YYYY-MM-DD) — dropped.",
                            trimmed
                        ));
                    } else if Some(trimmed.to_string()) != current.target_date {
                        diff.push(FieldDiff {
                            field: "targetDate".to_string(),
                            old: current.target_date.clone(),
                            new: Some(trimmed.to_string()),
                        });
                        draft.target_date = Some(trimmed.to_string());
                    }
                }

                if let Some(p) = args.priority {
                    if !(1..=3).contains(&p) {
                        warnings.push(format!(
                            "Priority {} is out of range (1-3) — dropped.",
                            p
                        ));
                    } else if p != current.priority {
                        diff.push(FieldDiff {
                            field: "priority".to_string(),
                            old: Some(current.priority.to_string()),
                            new: Some(p.to_string()),
                        });
                        draft.priority = p;
                    }
                }

                if let Some(s) = args.status_lifecycle.as_ref() {
                    let upper = s.trim().to_uppercase();
                    if !VALID_LIFECYCLE.contains(&upper.as_str()) {
                        warnings.push(format!(
                            "Status '{}' is unknown — valid: {}. Dropped.",
                            s,
                            VALID_LIFECYCLE.join(", ")
                        ));
                    } else if upper != current.status_lifecycle.to_uppercase() {
                        diff.push(FieldDiff {
                            field: "statusLifecycle".to_string(),
                            old: Some(current.status_lifecycle.clone()),
                            new: Some(upper.clone()),
                        });
                        draft.status_lifecycle = upper;
                    }
                }

                if let Some(d) = args.description.as_ref() {
                    let trimmed_owned: String = d.trim().to_string();
                    let new_desc = if trimmed_owned.is_empty() {
                        None
                    } else {
                        Some(trimmed_owned)
                    };
                    if new_desc != current.description {
                        diff.push(FieldDiff {
                            field: "description".to_string(),
                            old: current.description.clone(),
                            new: new_desc.clone(),
                        });
                        draft.description = new_desc;
                    }
                }

                let has_changes = !diff.is_empty();
                if !has_changes {
                    warnings
                        .push("Nothing to change — every field matched the current goal.".to_string());
                }

                Ok(UpdateGoalOutput {
                    draft,
                    current: snapshot(&current),
                    diff,
                    validation: ValidationResult {
                        resolved: true,
                        candidates: Vec::new(),
                        warnings,
                        is_valid: has_changes,
                        has_changes,
                    },
                })
            }
            Resolution::Ambiguous(candidates) => Ok(UpdateGoalOutput {
                draft: GoalUpdateDraft::default(),
                current: GoalSnapshot::default(),
                diff: Vec::new(),
                validation: ValidationResult {
                    resolved: false,
                    candidates: candidates.iter().map(snapshot).collect(),
                    warnings: vec![format!(
                        "\"{}\" matched {} goals. Ask the user which one they mean by title.",
                        args.goal_ref,
                        candidates.len()
                    )],
                    is_valid: false,
                    has_changes: false,
                },
            }),
            Resolution::NotFound => Ok(UpdateGoalOutput {
                draft: GoalUpdateDraft::default(),
                current: GoalSnapshot::default(),
                diff: Vec::new(),
                validation: ValidationResult {
                    resolved: false,
                    candidates: Vec::new(),
                    warnings: vec![format!(
                        "No goal matching \"{}\" found. Call get_goals to list goals first.",
                        args.goal_ref
                    )],
                    is_valid: false,
                    has_changes: false,
                },
            }),
            Resolution::Missing => Ok(UpdateGoalOutput {
                draft: GoalUpdateDraft::default(),
                current: GoalSnapshot::default(),
                diff: Vec::new(),
                validation: ValidationResult {
                    resolved: false,
                    candidates: Vec::new(),
                    warnings: vec![
                        "goalRef is required — pass the goal title or id.".to_string(),
                    ],
                    is_valid: false,
                    has_changes: false,
                },
            }),
        }
    }
}

impl<E: AiEnvironment + 'static> Tool for UpdateGoalTool<E> {
    const NAME: &'static str = "update_goal";

    type Error = AiError;
    type Args = UpdateGoalArgs;
    type Output = UpdateGoalOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description:
                "Edit an existing goal — title, target amount, target date, priority, lifecycle \
                 status, or description. Resolves the goal by id or title (exact or substring). \
                 Returns a DRAFT preview with a per-field diff the user confirms before the \
                 actual mutation. Always prefer this to delete+create when the user wants to \
                 adjust an existing goal — it preserves funding rules + plan history."
                    .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "goalRef": {
                        "type": "string",
                        "description": "Goal id or title match (exact or substring)."
                    },
                    "title": { "type": "string", "description": "New title (omit to keep current)." },
                    "targetAmount": { "type": "number", "description": "New target amount (omit to keep current)." },
                    "targetDate": { "type": "string", "description": "New ISO 8601 date YYYY-MM-DD, or empty to clear." },
                    "priority": { "type": "integer", "description": "1-3, 1 = highest." },
                    "statusLifecycle": {
                        "type": "string",
                        "enum": ["ACTIVE", "PAUSED", "DRAFT", "ACHIEVED", "ARCHIVED"],
                        "description": "New lifecycle state."
                    },
                    "description": { "type": "string", "description": "New description; empty string clears." }
                },
                "required": ["goalRef"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        self.build_output(args).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::test_env::{MockEnvironment, MockGoalService};
    use mizan_core::goals::Goal;

    fn make_goal(id: &str, title: &str) -> Goal {
        Goal {
            id: id.to_string(),
            goal_type: "RETIREMENT".to_string(),
            title: title.to_string(),
            description: Some("Originally drafted 2026-01".to_string()),
            target_amount: Some(1_000_000.0),
            status_lifecycle: "ACTIVE".to_string(),
            status_health: "ON_TRACK".to_string(),
            priority: 2,
            cover_image_key: None,
            currency: Some("USD".to_string()),
            start_date: None,
            target_date: Some("2055-01-01".to_string()),
            summary_current_value: Some(250_000.0),
            summary_progress: Some(25.0),
            projected_completion_date: None,
            projected_value_at_target_date: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            summary_target_amount: Some(1_000_000.0),
        }
    }

    fn tool_with_goals(goals: Vec<Goal>) -> UpdateGoalTool<MockEnvironment> {
        let mut env = MockEnvironment::new();
        env.goal_service = Arc::new(MockGoalService { goals });
        UpdateGoalTool::new(Arc::new(env))
    }

    #[tokio::test]
    async fn raises_target_amount_emits_diff() {
        let tool = tool_with_goals(vec![make_goal("g1", "Retirement")]);
        let out = tool
            .build_output(UpdateGoalArgs {
                goal_ref: "Retirement".into(),
                target_amount: Some(2_000_000.0),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(out.validation.is_valid);
        assert!(out.validation.has_changes);
        assert_eq!(out.draft.target_amount, Some(2_000_000.0));
        let amount_diff = out.diff.iter().find(|d| d.field == "targetAmount").unwrap();
        assert_eq!(amount_diff.new.as_deref(), Some("2000000"));
    }

    #[tokio::test]
    async fn no_op_warns_and_marks_invalid() {
        let tool = tool_with_goals(vec![make_goal("g1", "Retirement")]);
        let out = tool
            .build_output(UpdateGoalArgs {
                goal_ref: "Retirement".into(),
                target_amount: Some(1_000_000.0), // same as current
                priority: Some(2),                // same as current
                ..Default::default()
            })
            .await
            .unwrap();
        // No real changes → not valid for submit, warning surfaced.
        assert!(!out.validation.is_valid);
        assert!(!out.validation.has_changes);
        assert!(out
            .validation
            .warnings
            .iter()
            .any(|w| w.contains("Nothing to change")));
    }

    #[tokio::test]
    async fn invalid_date_warns_and_drops() {
        let tool = tool_with_goals(vec![make_goal("g1", "Retirement")]);
        let out = tool
            .build_output(UpdateGoalArgs {
                goal_ref: "Retirement".into(),
                target_date: Some("not-a-date".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        // Bad date is dropped; with no other changes, no-op warning fires.
        assert!(out
            .validation
            .warnings
            .iter()
            .any(|w| w.contains("not ISO 8601")));
        assert!(!out.validation.has_changes);
    }

    #[tokio::test]
    async fn unknown_status_is_dropped_with_warning() {
        let tool = tool_with_goals(vec![make_goal("g1", "Retirement")]);
        let out = tool
            .build_output(UpdateGoalArgs {
                goal_ref: "Retirement".into(),
                status_lifecycle: Some("RUMINATING".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(out
            .validation
            .warnings
            .iter()
            .any(|w| w.contains("unknown")));
        assert!(!out.validation.has_changes);
    }

    #[tokio::test]
    async fn priority_out_of_range_is_dropped() {
        let tool = tool_with_goals(vec![make_goal("g1", "Retirement")]);
        let out = tool
            .build_output(UpdateGoalArgs {
                goal_ref: "Retirement".into(),
                priority: Some(9),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(out
            .validation
            .warnings
            .iter()
            .any(|w| w.contains("out of range")));
        assert!(!out.validation.has_changes);
    }

    #[tokio::test]
    async fn ambiguous_match_blocks_confirm() {
        let tool = tool_with_goals(vec![
            make_goal("g1", "Retirement A"),
            make_goal("g2", "Retirement B"),
        ]);
        let out = tool
            .build_output(UpdateGoalArgs {
                goal_ref: "Retirement".into(),
                target_amount: Some(2_000_000.0),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(!out.validation.is_valid);
        assert_eq!(out.validation.candidates.len(), 2);
    }

    #[tokio::test]
    async fn missing_ref_blocks_confirm() {
        let tool = tool_with_goals(vec![make_goal("g1", "Retirement")]);
        let out = tool
            .build_output(UpdateGoalArgs {
                goal_ref: "  ".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(!out.validation.is_valid);
        assert!(out
            .validation
            .warnings
            .iter()
            .any(|w| w.contains("required")));
    }

    #[tokio::test]
    async fn clearing_target_date_emits_diff() {
        let tool = tool_with_goals(vec![make_goal("g1", "Retirement")]);
        let out = tool
            .build_output(UpdateGoalArgs {
                goal_ref: "Retirement".into(),
                target_date: Some("".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(out.validation.is_valid);
        assert!(out.draft.target_date.is_none());
        let diff = out.diff.iter().find(|d| d.field == "targetDate").unwrap();
        assert!(diff.new.is_none());
    }

    #[tokio::test]
    async fn status_change_normalises_case() {
        let tool = tool_with_goals(vec![make_goal("g1", "Retirement")]);
        let out = tool
            .build_output(UpdateGoalArgs {
                goal_ref: "Retirement".into(),
                status_lifecycle: Some("paused".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(out.validation.is_valid);
        assert_eq!(out.draft.status_lifecycle, "PAUSED");
    }
}
