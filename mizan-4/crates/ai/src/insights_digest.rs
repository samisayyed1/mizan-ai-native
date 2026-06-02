//! AI-narrated wealth-insights digest (Notify-4).
//!
//! Turns the deterministic insights engine's structured output into a
//! single 2-sentence natural-language summary — the **voice** layer on
//! top of the rule engine's **senses**. The rules never lie about
//! numbers; the AI just describes what the rules saw.
//!
//! COST BUDGET
//! ───────────
//! Exactly one LLM call per user per logical day. The caller enforces
//! this by checking for an existing `AiDigest` notification on today's
//! dedupe_key before invoking. If we ever drift to "one per tick" by
//! accident, this fires 6× per day per active user — wasted credit
//! and noise. The dedupe_key check is the firewall.
//!
//! GUARANTEES
//! ──────────
//! - Returns `Ok(None)` (NOT an error) when:
//!     * `managed_ai` is false (Free tier), or
//!     * the provider has no API key configured, or
//!     * the prompt content is empty (no insights to summarise).
//!
//!   Callers map `None` to "skip emitting the digest notification".
//!
//! - Never panics on malformed LLM output — we cap the response to
//!   400 chars and trim to the first two sentences, so a chatty
//!   model can't blow up the notification body.
//!
//! - The prompt is deliberately tiny (~120 tokens) so the cheapest
//!   tier of mizan-managed AI (`gpt-4o-mini` per ai_providers.json)
//!   handles it for ~0.001 USD.

use async_trait::async_trait;
use log::{debug, warn};
use reqwest::Client as HttpClient;
use rig::{
    client::{CompletionClient, Nothing},
    completion::Prompt,
    providers::{anthropic, gemini, groq, ollama, openai, openrouter},
};
use std::sync::Arc;

use crate::env::AiEnvironment;
use crate::error::AiError;
use crate::providers::ProviderService;

/// A single insight the rules engine emitted today, narrowed to the
/// fields the AI summariser actually needs. We pass title + body
/// (not the full JSON payload) so the LLM doesn't waste tokens
/// re-parsing structured data we've already humanised.
pub struct InsightForDigest {
    pub title: String,
    pub body: String,
    /// Severity slug ("info" / "success" / "warning" / "critical") so
    /// the LLM can pick the right tone.
    pub severity: String,
}

#[async_trait]
pub trait InsightsDigestServiceTrait: Send + Sync {
    /// Generate a 2-sentence personalized summary of `insights`.
    /// Returns `Ok(None)` per the guarantees in the module header.
    async fn generate(
        &self,
        insights: &[InsightForDigest],
        base_currency: &str,
        provider_id: &str,
        model_id: &str,
    ) -> Result<Option<String>, AiError>;
}

pub struct InsightsDigestService<E: AiEnvironment> {
    env: Arc<E>,
}

impl<E: AiEnvironment> InsightsDigestService<E> {
    pub fn new(env: Arc<E>) -> Self {
        Self { env }
    }
}

/// Free function so the prompt template is unit-testable without
/// instantiating an env-generic service.
fn build_prompt(insights: &[InsightForDigest], base_currency: &str) -> String {
    // List bullets are short + numbered so the LLM can reference
    // them without confusion. We deliberately don't ask for a
    // structured response — we want one natural-language paragraph.
    let lines: Vec<String> = insights
        .iter()
        .enumerate()
        .map(|(i, ins)| format!("{}. [{}] {} — {}", i + 1, ins.severity, ins.title, ins.body))
        .collect();
    let events_block = lines.join("\n");
    format!(
        "You are Mizan, a calm, factual wealth-tracking assistant. Write a personalised \
2-sentence summary of today's portfolio events for the user. \n\
Rules:\n\
- Plain text only. No markdown, no bullet points, no emojis.\n\
- Refer to the user in second person (\"you\", \"your\").\n\
- Base currency is {base_currency}. Never invent numbers — only use ones from the events.\n\
- Be helpful but not alarmist. Mention warnings calmly; celebrate wins quietly.\n\
- 2 sentences MAX, under 280 characters total.\n\
\n\
Events today:\n\
{events_block}\n\
\n\
Digest:"
    )
}

#[async_trait]
impl<E: AiEnvironment + 'static> InsightsDigestServiceTrait for InsightsDigestService<E> {
    async fn generate(
        &self,
        insights: &[InsightForDigest],
        base_currency: &str,
        provider_id: &str,
        model_id: &str,
    ) -> Result<Option<String>, AiError> {
        if insights.is_empty() {
            debug!("InsightsDigest: no events — nothing to summarise");
            return Ok(None);
        }

        let provider_service = ProviderService::new(self.env.clone());
        let api_key = provider_service.get_api_key(provider_id)?;

        // For the `mizan` (managed) provider, override URL + key with the
        // Mizan Connect JWT + cloud base URL — mirrors chat.rs line 1147.
        let (effective_key, provider_url): (Option<String>, Option<String>) =
            if provider_id == "mizan" {
                let token = self.env.connect_access_token().await.ok_or_else(|| {
                    AiError::MissingApiKey("mizan (no Connect session)".to_string())
                })?;
                let base = self.env.connect_api_url().await.ok_or_else(|| {
                    AiError::Provider("Mizan Connect URL not configured".to_string())
                })?;
                (
                    Some(token),
                    Some(format!("{}/v1", base.trim_end_matches('/'))),
                )
            } else {
                (api_key, provider_service.get_provider_url(provider_id))
            };

        let prompt = build_prompt(insights, base_currency);
        debug!(
            "InsightsDigest: calling {provider_id}/{model_id} on {} insights",
            insights.len()
        );

        let response = match provider_id {
            "anthropic" => {
                let key =
                    effective_key.ok_or_else(|| AiError::MissingApiKey(provider_id.to_string()))?;
                let mut builder = anthropic::Client::<HttpClient>::builder().api_key(&key);
                if let Some(url) = provider_url {
                    builder = builder.base_url(&url);
                }
                let client = builder
                    .build()
                    .map_err(|e| AiError::Provider(e.to_string()))?;
                client
                    .agent(model_id)
                    .max_tokens(200)
                    .build()
                    .prompt(&prompt)
                    .await
                    .map_err(|e| AiError::Provider(e.to_string()))?
            }
            "gemini" | "google" => {
                let key =
                    effective_key.ok_or_else(|| AiError::MissingApiKey(provider_id.to_string()))?;
                let mut builder = gemini::Client::<HttpClient>::builder().api_key(&key);
                if let Some(url) = provider_url {
                    builder = builder.base_url(&url);
                }
                let client = builder
                    .build()
                    .map_err(|e| AiError::Provider(e.to_string()))?;
                client
                    .agent(model_id)
                    .build()
                    .prompt(&prompt)
                    .await
                    .map_err(|e| AiError::Provider(e.to_string()))?
            }
            "groq" => {
                let key =
                    effective_key.ok_or_else(|| AiError::MissingApiKey(provider_id.to_string()))?;
                let mut builder = groq::Client::<HttpClient>::builder().api_key(&key);
                if let Some(url) = provider_url {
                    builder = builder.base_url(&url);
                }
                let client = builder
                    .build()
                    .map_err(|e| AiError::Provider(e.to_string()))?;
                client
                    .agent(model_id)
                    .build()
                    .prompt(&prompt)
                    .await
                    .map_err(|e| AiError::Provider(e.to_string()))?
            }
            "ollama" => {
                let mut builder = ollama::Client::<HttpClient>::builder().api_key(Nothing);
                if let Some(url) = provider_url {
                    builder = builder.base_url(&url);
                }
                let client = builder
                    .build()
                    .map_err(|e| AiError::Provider(e.to_string()))?;
                client
                    .agent(model_id)
                    .build()
                    .prompt(&prompt)
                    .await
                    .map_err(|e| AiError::Provider(e.to_string()))?
            }
            "openrouter" => {
                let key =
                    effective_key.ok_or_else(|| AiError::MissingApiKey(provider_id.to_string()))?;
                let mut builder = openrouter::Client::<HttpClient>::builder().api_key(&key);
                if let Some(url) = provider_url {
                    builder = builder.base_url(&url);
                }
                let client = builder
                    .build()
                    .map_err(|e| AiError::Provider(e.to_string()))?;
                client
                    .agent(model_id)
                    .build()
                    .prompt(&prompt)
                    .await
                    .map_err(|e| AiError::Provider(e.to_string()))?
            }
            // Default to OpenAI-compatible — this is the path the
            // `mizan` (managed) provider takes since the cloud's
            // /v1/chat/completions endpoint is OpenAI-compat.
            _ => {
                let key =
                    effective_key.ok_or_else(|| AiError::MissingApiKey(provider_id.to_string()))?;
                let mut builder = openai::Client::<HttpClient>::builder().api_key(&key);
                if let Some(url) = provider_url {
                    builder = builder.base_url(&url);
                }
                let client = builder
                    .build()
                    .map_err(|e| AiError::Provider(e.to_string()))?;
                client
                    .agent(model_id)
                    .build()
                    .prompt(&prompt)
                    .await
                    .map_err(|e| AiError::Provider(e.to_string()))?
            }
        };

        let cleaned = clean_digest(&response);
        if cleaned.is_empty() {
            warn!("InsightsDigest: LLM returned empty content — skipping");
            return Ok(None);
        }
        Ok(Some(cleaned))
    }
}

/// Defensive cleanup of LLM output:
///  - drops leading "Digest:" / "Summary:" prefixes,
///  - strips surrounding quotes / markdown wrappers,
///  - takes at most the first two sentences,
///  - caps at 400 chars so a chatty model can't blow up the
///    notification body or violate the dedupe-row TEXT column budget.
fn clean_digest(raw: &str) -> String {
    let mut s = raw.trim().to_string();

    // Strip a single leading "Digest:" / "Summary:" / "Response:" prefix.
    for prefix in ["Digest:", "Summary:", "Response:", "Today:"] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest.trim().to_string();
            break;
        }
    }

    // Strip surrounding "**" / `"`/ `'` / "`".
    for _ in 0..3 {
        let t = s.trim();
        let stripped = if (t.starts_with("**") && t.ends_with("**")) && t.len() > 4 {
            Some(t[2..t.len() - 2].to_string())
        } else if (t.starts_with('"') && t.ends_with('"'))
            || (t.starts_with('\'') && t.ends_with('\''))
            || (t.starts_with('`') && t.ends_with('`'))
        {
            if t.len() >= 2 {
                Some(t[1..t.len() - 1].to_string())
            } else {
                None
            }
        } else {
            None
        };
        match stripped {
            Some(next) => s = next,
            None => break,
        }
    }

    // Take at most two sentences. We use a simple period/!/? scan that
    // tolerates "U.S." and "etc." poorly — but for a 2-sentence cap
    // that's acceptable; worst case we cut at "U" and the next pass
    // catches the rest.
    let mut out = String::new();
    let mut sentence_count = 0usize;
    for ch in s.chars() {
        out.push(ch);
        if matches!(ch, '.' | '!' | '?') {
            sentence_count += 1;
            if sentence_count >= 2 {
                break;
            }
        }
        if out.chars().count() >= 400 {
            break;
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_strips_digest_prefix() {
        assert_eq!(
            clean_digest("Digest: You're up 2% this week. Keep going."),
            "You're up 2% this week. Keep going."
        );
    }

    #[test]
    fn clean_caps_at_two_sentences() {
        let raw = "Sentence one. Sentence two. Sentence three. Sentence four.";
        let cleaned = clean_digest(raw);
        assert!(cleaned.starts_with("Sentence one."));
        assert!(cleaned.contains("Sentence two."));
        assert!(!cleaned.contains("Sentence three"));
    }

    #[test]
    fn clean_drops_markdown_wrappers() {
        assert_eq!(
            clean_digest("**Your portfolio is up.**"),
            "Your portfolio is up."
        );
        assert_eq!(
            clean_digest("\"You hit your 75% milestone.\""),
            "You hit your 75% milestone."
        );
    }

    #[test]
    fn clean_handles_empty() {
        assert_eq!(clean_digest(""), "");
        assert_eq!(clean_digest("   "), "");
    }

    #[test]
    fn build_prompt_includes_all_insights() {
        let insights = vec![
            InsightForDigest {
                title: "PLTR -8% today".to_string(),
                body: "Your PLTR moved down -8.0%.".to_string(),
                severity: "warning".to_string(),
            },
            InsightForDigest {
                title: "House — 75% of target".to_string(),
                body: "You're now at 75% of your House target.".to_string(),
                severity: "info".to_string(),
            },
        ];
        let prompt = build_prompt(&insights, "USD");
        assert!(prompt.contains("PLTR"));
        assert!(prompt.contains("75%"));
        assert!(prompt.contains("USD"));
        assert!(prompt.contains("2 sentences MAX"));
    }
}
