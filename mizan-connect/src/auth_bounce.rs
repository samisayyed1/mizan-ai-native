//! `GET /auth/callback` — OAuth deep-link bounce page.
//!
//! Supabase + Google's identity platform require an HTTPS redirect target
//! — `mizan://` (the desktop's custom URL scheme) gets rejected with
//! `redirect_uri_mismatch`. Pointing Supabase at *this* endpoint instead
//! gives Google something it'll redirect to, and the tiny HTML response
//! we return immediately rebroadcasts the auth `code` + `state` into
//! `mizan://auth/callback?<query>` so the desktop's deep-link listener
//! can pick it up and complete the PKCE exchange.
//!
//! The HTML is intentionally minimal:
//! - One inline `<script>` that copies `window.location.search` and
//!   `window.location.hash` onto a fresh `mizan://auth/callback` URL.
//! - A `<noscript>` fallback with a manual "open Mizan" link so users
//!   with scripting disabled (rare, but: corporate sandboxes, screen-
//!   reader stress tests) don't end up stranded.
//! - A small, unbranded status message so the page doesn't feel like a
//!   blank tab during the (~50–200 ms) handoff.
//!
//! No state, no secrets, no database — this is intentionally a pure
//! HTML response so it can move to a CDN later if we want.
//!
//! Mounted in `server::routes()` at the workspace root (not under
//! `/api/v1` or `/v1`) because both Supabase + Google's allow-listed
//! redirect URI sets are top-level URLs.

use axum::{response::Html, routing::get, Router};

/// Tiny HTML bounce page. Static once compiled — the only dynamic bit is
/// the auth params lifted from `window.location` at render time, which
/// happens in the browser, not on the server.
// Raw string uses `r##" ... "##` (double-#) because the HTML contains
// `href="#"` for the manual-fallback link, and Rust would terminate
// `r#"..."#` early at the first `"#` sequence.
const BOUNCE_HTML: &str = r##"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <meta name="robots" content="noindex, nofollow" />
    <title>Signing in to Mizan…</title>
    <style>
      :root {
        color-scheme: light dark;
      }
      html, body {
        margin: 0;
        padding: 0;
        background: #fafaf7;
        color: #1f1f1d;
        font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI",
          Roboto, Helvetica, Arial, sans-serif;
        height: 100%;
      }
      @media (prefers-color-scheme: dark) {
        html, body { background: #0a0a09; color: #f0efe7; }
      }
      main {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        min-height: 100vh;
        padding: 24px;
        text-align: center;
      }
      .ring {
        width: 32px;
        height: 32px;
        border-radius: 50%;
        border: 3px solid currentColor;
        border-right-color: transparent;
        animation: spin 0.9s linear infinite;
        opacity: 0.6;
        margin-bottom: 16px;
      }
      h1 {
        font-size: 18px;
        font-weight: 600;
        margin: 0 0 8px;
      }
      p {
        font-size: 14px;
        margin: 0;
        opacity: 0.7;
        max-width: 320px;
        line-height: 1.5;
      }
      a {
        display: inline-block;
        margin-top: 12px;
        padding: 6px 12px;
        font-size: 14px;
        font-weight: 500;
        color: inherit;
        border: 1px solid currentColor;
        border-radius: 8px;
        text-decoration: none;
        opacity: 0.8;
      }
      a:hover { opacity: 1; }
      @keyframes spin { to { transform: rotate(360deg); } }
    </style>
  </head>
  <body>
    <main>
      <div class="ring" aria-hidden="true"></div>
      <h1>Returning to Mizan…</h1>
      <p id="status">If your browser doesn't switch back automatically, click the
      button below.</p>
      <a id="manual" href="#">Open Mizan</a>
    </main>
    <script>
      (function () {
        // Compose mizan://auth/callback?<query>#<hash> from the current
        // location. The desktop's deep-link listener picks the URL up
        // verbatim and runs handleAuthCallback() with it.
        var deepLink = "mizan://auth/callback" +
          (window.location.search || "") +
          (window.location.hash || "");

        var manual = document.getElementById("manual");
        if (manual) manual.setAttribute("href", deepLink);

        // Replace, not assign, so the user pressing Back from the
        // desktop app doesn't return to a stale callback URL with the
        // same auth code (Supabase rejects PKCE replays).
        try { window.location.replace(deepLink); }
        catch (e) { window.location.href = deepLink; }
      })();
    </script>
    <noscript>
      <p>Your browser blocked the automatic handoff. Open Mizan and try again.</p>
    </noscript>
  </body>
</html>"##;

async fn bounce() -> Html<&'static str> {
    Html(BOUNCE_HTML)
}

pub fn router<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new().route("/auth/callback", get(bounce))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::StatusCode;
    use tower::ServiceExt;

    #[tokio::test]
    async fn returns_html_with_bounce_script() {
        let app: Router<()> = router();
        let res = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/auth/callback")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = to_bytes(res.into_body(), 8192).await.unwrap();
        let body = std::str::from_utf8(&bytes).unwrap();
        // Critical bits: the bounce script and the noscript fallback link.
        assert!(body.contains("mizan://auth/callback"));
        assert!(body.contains("window.location.replace"));
        assert!(body.contains("<noscript>"));
    }
}
