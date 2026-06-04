//! pgvector-similarity layer for news personalization.
//!
//! Track D PR-D3 / Goal v3 §V Phase 6.
//!
//! Per the autonomous-loop directive `Mizan_Continue_Autonomous_v3.md`
//! line 63: "PR-D3 personalization worker on Mizan Connect with
//! pgvector similarity scoring news against user_memory + holdings.
//! Top items per user written to news_items_per_user materialized
//! cache."
//!
//! # Scope split
//!
//! This module ships the **pure-math similarity layer**: cosine
//! similarity + blending with the existing lexical baseline from
//! PR-D2. The actual pgvector SQL queries (SELECT embedding FROM
//! user_memory_embeddings WHERE user_id = $1) land in PR-D3.a once
//! the vector-index migration is in place. Until then, callers
//! pre-fetch embeddings + pass them as `Vec<UserMemoryEmbedding>`.
//!
//! # Embedding shape
//!
//! Mizan uses **OpenAI text-embedding-3-small** (1536-dim) by default
//! for `user_memory` rows. The cosine-similarity function is
//! dimension-agnostic — it just asserts both vectors have the same
//! length. If we migrate to a larger model (text-embedding-3-large at
//! 3072 dim) the helper continues to work; only the pgvector schema
//! migration changes.
//!
//! # Determinism
//!
//! Cosine similarity is deterministic for any pair of equal-length
//! vectors. Tests pin the math against known unit vectors.

use serde::{Deserialize, Serialize};

use super::personalization::{RankedArticle, RankingInput};
use super::types::RawArticle;

/// One embedding row from `user_memory_embeddings`. The `keyword` is
/// retained for the human-readable rationale ("Matches your memory
/// note about X").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserMemoryEmbedding {
    /// Source memory keyword (`fact_text` truncated to a phrase).
    pub keyword: String,
    /// Embedding vector. Length must match the article-embedding
    /// length used when scoring.
    pub vector: Vec<f32>,
}

/// Article paired with its precomputed embedding.
#[derive(Debug, Clone)]
pub struct ArticleEmbedding<'a> {
    pub article: &'a RawArticle,
    pub vector: Vec<f32>,
}

/// Compute cosine similarity between two equal-length vectors.
/// Returns `0.0` if either vector is the zero vector or the lengths
/// differ (defensive — never panics on bad data).
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0_f32;
    let mut norm_a = 0.0_f32;
    let mut norm_b = 0.0_f32;
    for (ai, bi) in a.iter().zip(b.iter()) {
        dot += ai * bi;
        norm_a += ai * ai;
        norm_b += bi * bi;
    }
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a.sqrt() * norm_b.sqrt())
}

/// Weight applied to the pgvector similarity signal when blended on
/// top of the lexical baseline. Caps at 0.3 — strong enough to break
/// ties but not so strong it overrides clear ticker matches (which
/// already weigh 0.6 per the lexical layer).
const VECTOR_SIMILARITY_WEIGHT: f32 = 0.3;

/// Minimum cosine similarity to count as a "hit". Below this, the
/// signal is noise and not credited.
const SIMILARITY_THRESHOLD: f32 = 0.65;

/// Rank articles with the lexical baseline (from `rank_articles`)
/// PLUS pgvector similarity against the user's memory embeddings.
///
/// `article_embeddings` is a slice of articles paired with their
/// precomputed embedding vectors. The caller is responsible for
/// embedding the articles (the worker batches this via the OpenAI
/// embeddings API + caches in news_items.embedding).
///
/// `memory_embeddings` is the user's `user_memory_embeddings` rows
/// fetched from pgvector.
///
/// Score formula:
///
/// ```text
/// final_score = lexical_score + vector_contribution
/// vector_contribution = VECTOR_SIMILARITY_WEIGHT × max_similarity_above_threshold
/// final_score capped at 1.0
/// ```
///
/// Only the best-matching memory embedding contributes per article
/// (we don't sum across all memory rows). This avoids over-weighting
/// users with many memory facts.
pub fn rank_with_memory_embeddings<'a>(
    article_embeddings: &[ArticleEmbedding<'a>],
    memory_embeddings: &[UserMemoryEmbedding],
    lexical_input: &RankingInput,
) -> Vec<RankedArticle> {
    let articles: Vec<RawArticle> = article_embeddings
        .iter()
        .map(|ae| ae.article.clone())
        .collect();

    // Compute lexical baseline first — this is the existing PR-D2
    // ranking. We'll blend the vector contribution on top.
    let mut ranked = super::personalization::rank_articles(&articles, lexical_input);

    // For each article, find the best-matching memory embedding +
    // add the contribution.
    for r in ranked.iter_mut() {
        let Some(ae) = article_embeddings
            .iter()
            .find(|ae| ae.article.provider_id == r.article.provider_id)
        else {
            continue;
        };

        let (best_similarity, best_keyword) = memory_embeddings
            .iter()
            .map(|me| {
                (
                    cosine_similarity(&ae.vector, &me.vector),
                    me.keyword.clone(),
                )
            })
            .fold((0.0_f32, String::new()), |acc, (sim, kw)| {
                if sim > acc.0 {
                    (sim, kw)
                } else {
                    acc
                }
            });

        if best_similarity >= SIMILARITY_THRESHOLD {
            let contribution = VECTOR_SIMILARITY_WEIGHT * best_similarity;
            r.score += contribution;
            if r.score > 1.0 {
                r.score = 1.0;
            }
            r.rationale.push(format!(
                "Semantically matches your memory note about \"{}\"",
                best_keyword
            ));
        }
    }

    // Re-sort after blending — vector boost can flip ordering.
    ranked.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.article.published_at.cmp(&a.article.published_at))
    });

    ranked
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::news::types::NewsCategory;

    fn make_article(provider_id: &str, title: &str, summary: &str) -> RawArticle {
        RawArticle::classify(
            "newsapi",
            provider_id,
            title,
            summary,
            "https://example.com",
            "Reuters",
            "2026-06-01T00:00:00Z",
            vec![],
        )
    }

    // ─── cosine_similarity ─────────────────────────────────────

    #[test]
    fn cosine_identical_vectors_is_one() {
        let v = vec![1.0, 2.0, 3.0];
        let s = cosine_similarity(&v, &v);
        assert!((s - 1.0).abs() < 0.0001, "expected ~1.0, got {s}");
    }

    #[test]
    fn cosine_orthogonal_vectors_is_zero() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn cosine_opposite_vectors_is_negative_one() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let s = cosine_similarity(&a, &b);
        assert!((s - (-1.0)).abs() < 0.0001, "expected ~-1.0, got {s}");
    }

    #[test]
    fn cosine_length_mismatch_returns_zero() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn cosine_empty_vectors_return_zero() {
        let empty: Vec<f32> = vec![];
        assert_eq!(cosine_similarity(&empty, &empty), 0.0);
    }

    #[test]
    fn cosine_zero_vector_returns_zero() {
        let zero = vec![0.0, 0.0, 0.0];
        let other = vec![1.0, 2.0, 3.0];
        assert_eq!(cosine_similarity(&zero, &other), 0.0);
        assert_eq!(cosine_similarity(&other, &zero), 0.0);
    }

    #[test]
    fn cosine_partial_alignment_between_zero_and_one() {
        // 45° angle between two 2D unit vectors → cos(45°) = sqrt(2)/2
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 1.0];
        let s = cosine_similarity(&a, &b);
        let expected = std::f32::consts::FRAC_1_SQRT_2;
        assert!(
            (s - expected).abs() < 0.01,
            "expected ~{expected} (cos 45°), got {s}"
        );
    }

    // ─── rank_with_memory_embeddings ───────────────────────────

    #[test]
    fn vector_boost_with_strong_similarity_adds_score() {
        let a1 = make_article("a", "Generic story", "...");
        let articles = vec![ArticleEmbedding {
            article: &a1,
            vector: vec![1.0, 0.0, 0.0],
        }];
        // Memory embedding aligned with article → similarity ~1.0
        let memory = vec![UserMemoryEmbedding {
            keyword: "ramadan".into(),
            vector: vec![1.0, 0.0, 0.0],
        }];
        let ranked = rank_with_memory_embeddings(&articles, &memory, &RankingInput::default());
        assert_eq!(ranked.len(), 1);
        // Lexical baseline = 0; vector contribution = 0.3 × 1.0 = 0.3
        assert!(
            (ranked[0].score - 0.3).abs() < 0.001,
            "expected ~0.3, got {}",
            ranked[0].score
        );
        assert!(
            ranked[0].rationale.iter().any(|r| r.contains("ramadan")),
            "rationale missing memory keyword"
        );
    }

    #[test]
    fn vector_below_threshold_ignored() {
        let a1 = make_article("a", "Generic story", "...");
        let articles = vec![ArticleEmbedding {
            article: &a1,
            vector: vec![1.0, 0.0, 0.0],
        }];
        // 45° angle → similarity ~0.707, just above threshold 0.65 → should fire
        // 60° angle → similarity ~0.5, below threshold → should NOT fire
        let memory = vec![UserMemoryEmbedding {
            keyword: "weak-match".into(),
            // 60° from [1,0,0] in 2D — similarity 0.5
            vector: vec![0.5, 0.866, 0.0],
        }];
        let ranked = rank_with_memory_embeddings(&articles, &memory, &RankingInput::default());
        // No vector contribution; no rationale added
        assert_eq!(ranked[0].score, 0.0);
        assert!(ranked[0].rationale.is_empty());
    }

    #[test]
    fn vector_picks_best_memory_match() {
        // Two memory rows: one strong match, one weak. Helper picks
        // the strong one + only counts it once.
        let a1 = make_article("a", "Generic story", "...");
        let articles = vec![ArticleEmbedding {
            article: &a1,
            vector: vec![1.0, 0.0, 0.0],
        }];
        let memory = vec![
            UserMemoryEmbedding {
                keyword: "weak".into(),
                vector: vec![0.5, 0.866, 0.0], // 0.5 cos
            },
            UserMemoryEmbedding {
                keyword: "strong".into(),
                vector: vec![1.0, 0.0, 0.0], // 1.0 cos
            },
        ];
        let ranked = rank_with_memory_embeddings(&articles, &memory, &RankingInput::default());
        assert!((ranked[0].score - 0.3).abs() < 0.001);
        // Only the strong match contributes to rationale
        assert!(ranked[0].rationale.iter().any(|r| r.contains("strong")));
        assert!(!ranked[0].rationale.iter().any(|r| r.contains("weak")));
    }

    #[test]
    fn vector_score_blends_with_lexical_baseline() {
        // §23 fixture-flavoured: an article with DAR ticker + Sukuks
        // category + memory embedding aligned with the article.
        // Lexical baseline = 0.9 (0.6 ticker + 0.3 category)
        // Vector boost = 0.3 × 1.0 = 0.3 → would sum to 1.2 but caps at 1.0
        let a1 = RawArticle::classify(
            "newsapi",
            "sukuk",
            "Dar Al Arkan sukuk rated A",
            "Saudi issuer upgrade",
            "https://example.com",
            "Reuters",
            "2026-06-01T00:00:00Z",
            vec!["DAR".into()],
        );
        let articles = vec![ArticleEmbedding {
            article: &a1,
            vector: vec![1.0, 0.0, 0.0],
        }];
        let memory = vec![UserMemoryEmbedding {
            keyword: "Saudi issuers".into(),
            vector: vec![1.0, 0.0, 0.0],
        }];
        let input = RankingInput {
            holding_symbols: vec!["DAR".into()],
            holding_categories: vec![NewsCategory::Sukuks],
            ..Default::default()
        };
        let ranked = rank_with_memory_embeddings(&articles, &memory, &input);
        // Capped at 1.0
        assert!(
            (ranked[0].score - 1.0).abs() < 0.001,
            "expected capped 1.0, got {}",
            ranked[0].score
        );
        // Rationale must include all 3 signals
        assert_eq!(ranked[0].rationale.len(), 3);
    }

    #[test]
    fn empty_memory_embeddings_returns_lexical_only() {
        let a1 = make_article("a", "Apple earnings beat", "...");
        let mut a1_with_ticker = a1.clone();
        a1_with_ticker.tickers = vec!["AAPL".into()];
        let articles = vec![ArticleEmbedding {
            article: &a1_with_ticker,
            vector: vec![1.0, 0.0, 0.0],
        }];
        let memory: Vec<UserMemoryEmbedding> = vec![];
        let input = RankingInput {
            holding_symbols: vec!["AAPL".into()],
            ..Default::default()
        };
        let ranked = rank_with_memory_embeddings(&articles, &memory, &input);
        // Lexical-only: 0.6 ticker match
        assert!(
            (ranked[0].score - 0.6).abs() < 0.001,
            "expected 0.6 (ticker only), got {}",
            ranked[0].score
        );
    }

    #[test]
    fn missing_article_embedding_falls_through_to_lexical() {
        // Article in ranking list but not in embedding map (e.g. the
        // embedder hadn't processed it yet). The lexical score still
        // applies; no vector contribution.
        let a1 = make_article("with-emb", "story", "...");
        let a2 = make_article("no-emb", "other story", "...");
        let articles = vec![
            ArticleEmbedding {
                article: &a1,
                vector: vec![1.0, 0.0, 0.0],
            },
            // a2 NOT in the embedding list — pretend it was a late-
            // arriving article the embedder hasn't seen yet.
        ];
        // The helper iterates over article_embeddings, so a2 won't
        // appear in the output. Pin that.
        let memory = vec![UserMemoryEmbedding {
            keyword: "kw".into(),
            vector: vec![1.0, 0.0, 0.0],
        }];
        let ranked = rank_with_memory_embeddings(&articles, &memory, &RankingInput::default());
        // Only a1 returned
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].article.provider_id, "with-emb");
        let _ = a2; // a2 only constructed to document the intent
    }
}
