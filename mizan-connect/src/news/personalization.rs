//! News personalization — Track D PR-D2 (foundation) → PR-D3 (full vector
//! similarity over `user_memory`).
//!
//! Per autonomous-loop directive `Mizan_Continue_Autonomous.md`:
//!
//! > PR-D3 personalization worker scoring news against user_memory +
//! > holdings via vector similarity.
//!
//! PR-D2 ships the **deterministic lexical layer** so the relevance-tab
//! endpoint can return meaningful rankings against the §23 reference
//! user's portfolio without the vector store in the loop. PR-D3 then
//! threads in the pgvector-backed `user_memory` similarity score that
//! biases this baseline.
//!
//! # Scoring model
//!
//! Each article scores against the user's `RankingInput` by summing
//! contributions from three signals:
//!
//! 1. **Ticker overlap** — if the article carries a ticker matching
//!    a user holding's symbol, +0.6.
//! 2. **Category-of-holding** — if the article's category matches a
//!    category the user has positions in, +0.3.
//! 3. **Memory keyword** — if the article body contains a keyword
//!    declared in the user's memory facts (PR-D3 replaces with
//!    vector similarity), +0.1.
//!
//! Scores cap at 1.0. Articles with score 0.0 are still returned by
//! `rank_articles` (with score=0) so the relevant-tab caller can
//! fall back to global rotation when personalization yields nothing —
//! the §23 user's morning-coffee newsletter should always have content.
//!
//! # Determinism
//!
//! `rank_articles` is a pure function over its inputs. Tests pin every
//! branch. No I/O, no randomness, no clock reads.

use serde::{Deserialize, Serialize};

use super::types::{NewsCategory, RawArticle};

/// Inputs for the ranking step.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RankingInput {
    /// Tickers the user holds (upper-cased, deduplicated).
    pub holding_symbols: Vec<String>,
    /// Categories the user has positions in (derived from the asset-class
    /// classifier on the desktop side).
    pub holding_categories: Vec<NewsCategory>,
    /// Keywords from the user's memory facts (PR-D3 swaps for vector
    /// similarity; lexical is the stop-gap so D2 ships standalone).
    pub memory_keywords: Vec<String>,
}

/// A ranked article — original `RawArticle` plus its computed score
/// + the human-readable rationale that powers the "Why this matters
///   to you" sub-label.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankedArticle {
    pub article: RawArticle,
    pub score: f32,
    /// Rationale strings, one per matched signal. Ordered by signal
    /// weight (ticker > category > memory keyword) so the most
    /// salient reason renders first.
    pub rationale: Vec<String>,
}

const TICKER_WEIGHT: f32 = 0.6;
const CATEGORY_WEIGHT: f32 = 0.3;
const MEMORY_WEIGHT: f32 = 0.1;

/// Rank a slice of articles against the user's profile. Returns a new
/// `Vec<RankedArticle>` in descending-by-score order. Ties break by
/// `published_at` descending (newer first) so the freshest of two
/// equally-scoring articles wins.
pub fn rank_articles(articles: &[RawArticle], input: &RankingInput) -> Vec<RankedArticle> {
    let holding_symbols: Vec<String> = input
        .holding_symbols
        .iter()
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty())
        .collect();
    let holding_categories: std::collections::HashSet<NewsCategory> =
        input.holding_categories.iter().copied().collect();
    let memory_keywords: Vec<String> = input
        .memory_keywords
        .iter()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    let mut ranked: Vec<RankedArticle> = articles
        .iter()
        .map(|article| {
            let mut score = 0.0_f32;
            let mut rationale = Vec::new();

            // Ticker overlap — strongest signal.
            let article_tickers: Vec<String> = article
                .tickers
                .iter()
                .map(|t| t.trim().to_uppercase())
                .filter(|t| !t.is_empty())
                .collect();
            for ticker in &article_tickers {
                if holding_symbols.iter().any(|s| s == ticker) {
                    score += TICKER_WEIGHT;
                    rationale.push(format!("You hold {}", ticker));
                    break; // Only count one ticker match.
                }
            }

            // Category alignment.
            if holding_categories.contains(&article.category) {
                score += CATEGORY_WEIGHT;
                rationale.push(format!("Touches your {:?} positions", article.category));
            }

            // Memory keyword.
            let blob = format!(
                "{} {}",
                article.title.to_lowercase(),
                article.summary.to_lowercase()
            );
            for kw in &memory_keywords {
                if blob.contains(kw) {
                    score += MEMORY_WEIGHT;
                    rationale.push(format!("Matches your memory note \"{}\"", kw));
                    break; // Only count one memory match.
                }
            }

            // Cap score at 1.0
            if score > 1.0 {
                score = 1.0;
            }

            RankedArticle {
                article: article.clone(),
                score,
                rationale,
            }
        })
        .collect();

    // Sort: score desc, then published_at desc (string sort works for ISO-8601).
    ranked.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.article.published_at.cmp(&a.article.published_at))
    });

    ranked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make(
        provider_id: &str,
        title: &str,
        summary: &str,
        tickers: Vec<&str>,
        time: &str,
    ) -> RawArticle {
        RawArticle::classify(
            "newsapi",
            provider_id,
            title,
            summary,
            "https://example.com",
            "Reuters",
            time,
            tickers.into_iter().map(String::from).collect(),
        )
    }

    #[test]
    fn empty_input_returns_zero_scores() {
        let articles = vec![make(
            "a",
            "Generic story",
            "...",
            vec![],
            "2026-06-01T00:00:00Z",
        )];
        let ranked = rank_articles(&articles, &RankingInput::default());
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].score, 0.0);
        assert!(ranked[0].rationale.is_empty());
    }

    #[test]
    fn ticker_match_drives_top_score() {
        let articles = vec![
            make(
                "a",
                "Apple beats earnings",
                "...",
                vec!["AAPL"],
                "2026-06-01T00:00:00Z",
            ),
            make(
                "b",
                "Unrelated",
                "...",
                vec!["MSFT"],
                "2026-06-02T00:00:00Z",
            ),
        ];
        let input = RankingInput {
            holding_symbols: vec!["AAPL".into()],
            ..Default::default()
        };
        let ranked = rank_articles(&articles, &input);
        assert_eq!(ranked[0].article.provider_id, "a");
        assert!(ranked[0].score >= 0.6);
        assert_eq!(ranked[0].rationale[0], "You hold AAPL");
    }

    #[test]
    fn ticker_match_is_case_and_whitespace_insensitive() {
        let articles = vec![make(
            "a",
            "Earnings update",
            "...",
            vec!["aapl"],
            "2026-06-01T00:00:00Z",
        )];
        let input = RankingInput {
            holding_symbols: vec!["  AAPL  ".into()],
            ..Default::default()
        };
        let ranked = rank_articles(&articles, &input);
        assert_eq!(ranked[0].score, TICKER_WEIGHT);
    }

    #[test]
    fn category_alignment_score() {
        let articles = vec![make(
            "a",
            "Singapore housing prices rise",
            "Property tax shake-up",
            vec![],
            "2026-06-01T00:00:00Z",
        )];
        let input = RankingInput {
            holding_categories: vec![NewsCategory::RealEstate],
            ..Default::default()
        };
        let ranked = rank_articles(&articles, &input);
        assert_eq!(ranked[0].score, CATEGORY_WEIGHT);
        assert!(ranked[0].rationale[0].contains("RealEstate"));
    }

    #[test]
    fn memory_keyword_score() {
        let articles = vec![make(
            "a",
            "Ramadan giving up",
            "Donations to mosques spike",
            vec![],
            "2026-06-01T00:00:00Z",
        )];
        let input = RankingInput {
            memory_keywords: vec!["ramadan".into()],
            ..Default::default()
        };
        let ranked = rank_articles(&articles, &input);
        assert_eq!(ranked[0].score, MEMORY_WEIGHT);
        assert!(ranked[0].rationale[0].contains("ramadan"));
    }

    #[test]
    fn all_three_signals_sum_and_cap() {
        let articles = vec![make(
            "a",
            "Dar Al Arkan sukuk maturity",
            "Investors weigh refinancing during Ramadan",
            vec!["DAR"],
            "2026-06-01T00:00:00Z",
        )];
        let input = RankingInput {
            holding_symbols: vec!["DAR".into()],
            holding_categories: vec![NewsCategory::Sukuks],
            memory_keywords: vec!["ramadan".into()],
        };
        let ranked = rank_articles(&articles, &input);
        let s = ranked[0].score;
        assert!((s - 1.0).abs() < 0.001, "expected score ~1.0, got {s}");
        assert_eq!(ranked[0].rationale.len(), 3);
    }

    #[test]
    fn ties_break_by_published_at_desc() {
        let articles = vec![
            make(
                "older",
                "AAPL story",
                "...",
                vec!["AAPL"],
                "2026-06-01T00:00:00Z",
            ),
            make(
                "newer",
                "AAPL update",
                "...",
                vec!["AAPL"],
                "2026-06-03T00:00:00Z",
            ),
        ];
        let input = RankingInput {
            holding_symbols: vec!["AAPL".into()],
            ..Default::default()
        };
        let ranked = rank_articles(&articles, &input);
        assert_eq!(ranked[0].article.provider_id, "newer");
    }

    #[test]
    fn empty_articles_returns_empty_vec() {
        let ranked = rank_articles(&[], &RankingInput::default());
        assert!(ranked.is_empty());
    }

    #[test]
    fn empty_holding_symbol_strings_ignored() {
        let articles = vec![make(
            "a",
            "AAPL story",
            "...",
            vec!["AAPL"],
            "2026-06-01T00:00:00Z",
        )];
        let input = RankingInput {
            holding_symbols: vec!["   ".into(), "".into(), "AAPL".into()],
            ..Default::default()
        };
        let ranked = rank_articles(&articles, &input);
        assert_eq!(ranked[0].score, TICKER_WEIGHT);
    }

    #[test]
    fn s23_reference_user_sukuk_headline_outranks_generic_macro() {
        let articles = vec![
            make(
                "macro",
                "Federal Reserve hikes rates",
                "Inflation prints",
                vec![],
                "2026-06-01T00:00:00Z",
            ),
            make(
                "sukuk",
                "Dar Al Arkan sukuk rated A by Moody's",
                "Saudi issuer benefits from oil sector tailwind",
                vec!["DAR"],
                "2026-06-01T00:00:00Z",
            ),
        ];
        let input = RankingInput {
            holding_symbols: vec!["DAR".into(), "EMAAR".into(), "SOBHA".into()],
            holding_categories: vec![NewsCategory::Sukuks, NewsCategory::Bonds],
            memory_keywords: vec!["zakat".into(), "ramadan".into()],
        };
        let ranked = rank_articles(&articles, &input);
        assert_eq!(ranked[0].article.provider_id, "sukuk");
        assert!(ranked[0].score >= 0.9);
        assert!(ranked[1].score < 0.1);
    }

    #[test]
    fn empty_memory_keywords_with_blank_strings_skipped() {
        let articles = vec![make(
            "a",
            "Ramadan story",
            "...",
            vec![],
            "2026-06-01T00:00:00Z",
        )];
        let input = RankingInput {
            memory_keywords: vec!["   ".into(), "".into()],
            ..Default::default()
        };
        let ranked = rank_articles(&articles, &input);
        assert_eq!(ranked[0].score, 0.0);
    }
}
