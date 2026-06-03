//! HTTP handlers for the AAOIFI screening endpoints — ADR 0012.
//!
//! The handlers wrap the pure `aaoifi_rules` engine. They do NOT yet
//! persist verdicts (per `mod.rs` doc: persistence lands in PR-E4.b
//! once the holdings model is reachable from mizan-connect). For now
//! the `POST /screen` endpoint is a stateless oracle the desktop +
//! agent both consume.

use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

use crate::auth::AuthenticatedUser;
use crate::state::AppState;

use super::aaoifi_rules::{screen, ScreeningInput, ScreeningRationale, Verdict};

/// Envelope for `POST /v1/sharia/screen`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenResponse {
    pub symbol: String,
    pub verdict: Verdict,
    pub rationale: ScreeningRationale,
}

/// `POST /v1/sharia/screen` — run both screens against a caller-
/// supplied input bundle and return the verdict.
///
/// The request payload IS [`ScreeningInput`]; the response embeds
/// the verdict + rationale alongside the input symbol so the caller
/// can match results back to a holdings batch.
pub async fn screen_handler(
    State(_state): State<AppState>,
    _auth: AuthenticatedUser,
    Json(input): Json<ScreeningInput>,
) -> Json<ScreenResponse> {
    let symbol = input.symbol.clone();
    let (verdict, rationale) = screen(&input);
    Json(ScreenResponse {
        symbol,
        verdict,
        rationale,
    })
}

/// `GET /v1/sharia/status/:symbol` — return the cached verdict for a
/// symbol if one exists. Currently always returns `Unrated` because
/// the persistence layer lands in PR-E4.b; this surface ships now so
/// the desktop + agent code paths can wire against a stable URL.
pub async fn status_handler(
    State(_state): State<AppState>,
    _auth: AuthenticatedUser,
    Path(symbol): Path<String>,
) -> Json<ScreenResponse> {
    // The "screen never ran for this symbol" case maps to Unrated +
    // a rationale that names exactly that. When PR-E4.b adds the
    // cache, this branch becomes "return persisted row if present;
    // else Unrated with cache_miss reason".
    Json(ScreenResponse {
        symbol,
        verdict: Verdict::Unrated,
        rationale: ScreeningRationale {
            reasons: vec!["cache_miss".to_string()],
            purification_ratio: None,
        },
    })
}
