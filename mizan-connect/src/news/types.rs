//! Shared news types — Track D PR-D2 / Goal v3 §V Phase 6.
//!
//! Every provider client returns `RawArticle` values which the
//! personalization layer then ranks. The shape is intentionally
//! minimal (title / summary / URL / source / published time / tickers)
//! so adding a new provider doesn't ripple through the type tree.

use serde::{Deserialize, Serialize};

/// Tab selector — controls which slice of news a client requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NewsTab {
    /// Articles ranked against the user's holdings + `user_memory`.
    Relevant,
    /// All market news without personalization filter applied.
    Global,
}

impl NewsTab {
    /// Parse the `?tab=` query string. Unknown values default to
    /// `Global` (the safest fallback — never silently personalises).
    pub fn parse(raw: &str) -> Self {
        match raw.to_lowercase().trim() {
            "relevant" => Self::Relevant,
            _ => Self::Global,
        }
    }
}

/// Topical bucket. Used to weight personalization (a "sukuks" article
/// against a user with sukuk holdings scores higher than the same
/// article against a crypto-only user).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum NewsCategory {
    Equities,
    Bonds,
    Sukuks,
    Crypto,
    Commodities,
    Forex,
    RealEstate,
    Macro,
    Regulatory,
    Other,
}

impl NewsCategory {
    /// Classify an article by simple lexical match over title +
    /// summary. The classifier is intentionally conservative — when
    /// no signal beats a low threshold, return `Other`. Personalization
    /// upgrades the score later when the user has positions in the
    /// matched category.
    pub fn classify(title: &str, summary: &str) -> Self {
        let blob = format!("{} {}", title.to_lowercase(), summary.to_lowercase());

        // Order matters: more specific buckets first so a "sukuk" headline
        // doesn't get mis-classified as generic Bonds.
        if blob.contains("sukuk") || blob.contains("islamic bond") {
            return Self::Sukuks;
        }
        if blob.contains("bitcoin")
            || blob.contains("ethereum")
            || blob.contains("crypto")
            || blob.contains("solana")
            || blob.contains("blockchain")
            || blob.contains("stablecoin")
        {
            return Self::Crypto;
        }
        if blob.contains("gold")
            || blob.contains("silver")
            || blob.contains("platinum")
            || blob.contains("commodit")
        {
            return Self::Commodities;
        }
        if blob.contains("forex")
            || blob.contains("currency")
            || blob.contains("fx ")
            || blob.contains("usd/")
            || blob.contains("eur/")
        {
            return Self::Forex;
        }
        if blob.contains("real estate")
            || blob.contains("property")
            || blob.contains("housing")
            || blob.contains("mortgage")
        {
            return Self::RealEstate;
        }
        if blob.contains("bond") || blob.contains("yield") || blob.contains("treasury") {
            return Self::Bonds;
        }
        if blob.contains("stock")
            || blob.contains("equity")
            || blob.contains("share price")
            || blob.contains("earnings")
        {
            return Self::Equities;
        }
        if blob.contains("regulat")
            || blob.contains("sec ")
            || blob.contains("compliance")
            || blob.contains("fatwa")
            || blob.contains("aaoifi")
        {
            return Self::Regulatory;
        }
        if blob.contains("inflation")
            || blob.contains("recession")
            || blob.contains("gdp")
            || blob.contains("central bank")
            || blob.contains("federal reserve")
            || blob.contains("ecb")
            || blob.contains("mas ")
        {
            return Self::Macro;
        }
        Self::Other
    }
}

/// A provider-agnostic article shape. Personalization scores ranges
/// over these, and the desktop sync layer flattens them into the
/// per-user `news_items` rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawArticle {
    /// Stable per-provider article id (URL is the safest fallback —
    /// uniqueness across providers is enforced upstream by hashing
    /// provider + id when materialising into `news_items`).
    pub provider_id: String,
    /// Provider name — `"newsapi"`, `"benzinga"`, etc.
    pub provider: String,
    /// Article title.
    pub title: String,
    /// Article body summary (truncated to <= 800 chars by the provider
    /// client — long-form text doesn't belong in news cards).
    pub summary: String,
    /// Permalink to the source.
    pub url: String,
    /// Publisher / outlet name.
    pub source: String,
    /// ISO-8601 publish timestamp from the provider.
    pub published_at: String,
    /// Best-effort ticker extraction. Provider clients should populate
    /// when the API exposes it; empty vec when not.
    pub tickers: Vec<String>,
    /// Lexically classified category.
    pub category: NewsCategory,
}

impl RawArticle {
    /// Convenience constructor with category auto-classification —
    /// providers use this to avoid re-deriving the category every
    /// time.
    #[allow(clippy::too_many_arguments)]
    pub fn classify(
        provider: impl Into<String>,
        provider_id: impl Into<String>,
        title: impl Into<String>,
        summary: impl Into<String>,
        url: impl Into<String>,
        source: impl Into<String>,
        published_at: impl Into<String>,
        tickers: Vec<String>,
    ) -> Self {
        let title = title.into();
        let summary = summary.into();
        let category = NewsCategory::classify(&title, &summary);
        Self {
            provider: provider.into(),
            provider_id: provider_id.into(),
            title,
            summary,
            url: url.into(),
            source: source.into(),
            published_at: published_at.into(),
            tickers,
            category,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn news_tab_parses_relevant() {
        assert_eq!(NewsTab::parse("relevant"), NewsTab::Relevant);
        assert_eq!(NewsTab::parse("RELEVANT"), NewsTab::Relevant);
        assert_eq!(NewsTab::parse("  Relevant  "), NewsTab::Relevant);
    }

    #[test]
    fn news_tab_defaults_to_global_for_unknown() {
        assert_eq!(NewsTab::parse("global"), NewsTab::Global);
        assert_eq!(NewsTab::parse(""), NewsTab::Global);
        assert_eq!(NewsTab::parse("trending"), NewsTab::Global);
        assert_eq!(NewsTab::parse("for-you"), NewsTab::Global);
    }

    #[test]
    fn category_classify_sukuks_beats_generic_bond() {
        assert_eq!(
            NewsCategory::classify("Dar Al Arkan sukuk rated A", "the issuer's bond is..."),
            NewsCategory::Sukuks
        );
    }

    #[test]
    fn category_classify_crypto_matches_variants() {
        assert_eq!(
            NewsCategory::classify("Bitcoin rallies", ""),
            NewsCategory::Crypto
        );
        assert_eq!(
            NewsCategory::classify("Ethereum upgrade", ""),
            NewsCategory::Crypto
        );
        assert_eq!(
            NewsCategory::classify("USDC stablecoin issuance", ""),
            NewsCategory::Crypto
        );
    }

    #[test]
    fn category_classify_commodities_metals() {
        assert_eq!(
            NewsCategory::classify("Gold hits all-time high", ""),
            NewsCategory::Commodities
        );
        assert_eq!(
            NewsCategory::classify("Silver futures gain", ""),
            NewsCategory::Commodities
        );
    }

    #[test]
    fn category_classify_forex_pair_pattern() {
        assert_eq!(
            NewsCategory::classify("USD/SGD pair weakens", ""),
            NewsCategory::Forex
        );
        assert_eq!(
            NewsCategory::classify("Forex markets tense", ""),
            NewsCategory::Forex
        );
    }

    #[test]
    fn category_classify_real_estate_property_or_housing() {
        assert_eq!(
            NewsCategory::classify("Singapore housing prices rise", ""),
            NewsCategory::RealEstate
        );
        assert_eq!(
            NewsCategory::classify("Property tax shake-up", ""),
            NewsCategory::RealEstate
        );
    }

    #[test]
    fn category_classify_bonds_generic() {
        assert_eq!(
            NewsCategory::classify("Treasury yields climb", ""),
            NewsCategory::Bonds
        );
    }

    #[test]
    fn category_classify_equities_keywords() {
        assert_eq!(
            NewsCategory::classify("Apple earnings beat", ""),
            NewsCategory::Equities
        );
        assert_eq!(
            NewsCategory::classify("Tesla share price drops", ""),
            NewsCategory::Equities
        );
    }

    #[test]
    fn category_classify_regulatory_matches_aaoifi() {
        assert_eq!(
            NewsCategory::classify("AAOIFI updates screening criteria", ""),
            NewsCategory::Regulatory
        );
        assert_eq!(
            NewsCategory::classify("SEC issues new compliance rule", ""),
            NewsCategory::Regulatory
        );
    }

    #[test]
    fn category_classify_macro_central_banks() {
        assert_eq!(
            NewsCategory::classify("Federal Reserve hikes rates", ""),
            NewsCategory::Macro
        );
        assert_eq!(
            NewsCategory::classify("Inflation prints hot", ""),
            NewsCategory::Macro
        );
        assert_eq!(
            NewsCategory::classify("MAS issues quarterly update", ""),
            NewsCategory::Macro
        );
    }

    #[test]
    fn category_classify_other_fallback() {
        assert_eq!(
            NewsCategory::classify("Local sports team wins championship", ""),
            NewsCategory::Other
        );
    }

    #[test]
    fn classify_constructor_populates_category() {
        let a = RawArticle::classify(
            "newsapi",
            "abc-1",
            "Emaar sukuk maturity coming up",
            "Investors weigh refinancing options",
            "https://example.com/article",
            "Reuters",
            "2026-06-01T00:00:00Z",
            vec!["EMAAR".into()],
        );
        assert_eq!(a.category, NewsCategory::Sukuks);
        assert_eq!(a.tickers, vec!["EMAAR".to_string()]);
    }
}
