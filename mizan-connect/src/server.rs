//! Axum router composition and server bootstrap.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::http::{HeaderName, HeaderValue, Method};
use axum::Router;
use tower::util::option_layer;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::GovernorLayer;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

use crate::config::Config;
use crate::middleware::request_id::RequestIdLayer;
use crate::middleware::{security_headers, timeout};
use crate::state::AppState;

/// Maximum request body size accepted by the server.
const MAX_BODY_BYTES: usize = 1024 * 1024; // 1 MB

/// Build the fully-wired Axum router.
pub fn build_app(state: AppState) -> Router {
    let cors = build_cors(state.config());
    let governor = GovernorLayer {
        config: build_rate_limiter_config(state.config()),
    };
    let security_headers::SecurityHeaderLayers {
        nosniff,
        frame,
        referrer,
        permissions,
        cross_domain,
        hsts,
    } = security_headers::layers(state.config().app_env);

    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(|req: &axum::http::Request<_>| {
            let request_id = req
                .headers()
                .get(crate::middleware::request_id::HEADER_NAME)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            // user_id is initialised to "" and populated later by the
            // AuthenticatedUser extractor via tracing::Span::current()
            // .record("user_id", ...) — declaring the field here means
            // it shows up in structured logs even for anonymous routes
            // (as an empty string) without a second span layer.
            tracing::info_span!(
                "http_request",
                method = %req.method(),
                uri = %req.uri(),
                version = ?req.version(),
                request_id = %request_id,
                user_id = tracing::field::Empty,
                status = tracing::field::Empty,
                latency_ms = tracing::field::Empty,
            )
        })
        .on_response(
            |res: &axum::http::Response<_>,
             latency: std::time::Duration,
             span: &tracing::Span| {
                span.record("status", res.status().as_u16());
                span.record("latency_ms", latency.as_millis() as u64);
                tracing::info!(
                    status = res.status().as_u16(),
                    latency_ms = latency.as_millis() as u64,
                    "request completed"
                );
            },
        );

    // Legacy versioned mount. Kept live for tests and any internal callers.
    let v1 = Router::new()
        .merge(crate::users::router())
        .merge(crate::teams::router())
        .merge(crate::billing::router())
        .with_state(state.clone());

    // Production mount. The desktop client (samisayyed1/mizan-4) calls
    // `/api/v1/...` — a relic of upstream Wealthfolio's API shape that we
    // can't change because the desktop's Rust HTTP client is out of scope
    // for backend chunks. We expose:
    //   - /api/v1/me                            (alias of /v1/me)
    //   - /api/v1/user/me                       (desktop's actual call)
    //   - /api/v1/subscription/plans            (Chunk 4)
    //   - /api/v1/sync/plaid/*                  (Gold live sync via Plaid)
    //   - /api/v1/billing/*                     (Chunk 4 — Stripe checkout/portal)
    //   - /api/v1/usage                         (Chunk 4 — usage ledger)
    //   - /api/v1/stripe/webhook                (Chunk 4 — public, signature-verified)
    let api_v1 = Router::new()
        .merge(crate::users::router())
        .route(
            "/user/me",
            axum::routing::get(crate::users::handlers::get_me),
        )
        .merge(crate::teams::router())
        .merge(crate::billing::plans_router())
        .merge(crate::billing::router())
        .merge(crate::plaid::router())
        .with_state(state.clone());

    Router::new()
        .merge(crate::health::router())
        .nest("/v1", v1)
        .nest("/api/v1", api_v1)
        .with_state(state)
        // The order matters: outermost layer is added last.
        .layer(nosniff)
        .layer(frame)
        .layer(referrer)
        .layer(permissions)
        .layer(cross_domain)
        .layer(option_layer(hsts))
        .layer(cors)
        .layer(governor)
        .layer(RequestBodyLimitLayer::new(MAX_BODY_BYTES))
        .layer(timeout::layer())
        .layer(RequestIdLayer)
        .layer(trace_layer)
        .layer(sentry_tower::NewSentryLayer::<axum::http::Request<_>>::new_from_top())
        .layer(sentry_tower::SentryHttpLayer::with_transaction())
}

fn build_cors(config: &Config) -> CorsLayer {
    let mut layer = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
            HeaderName::from_static("x-request-id"),
        ])
        .max_age(Duration::from_secs(60 * 10));

    for origin in &config.cors_allowed_origins {
        match origin.parse::<HeaderValue>() {
            Ok(value) => layer = layer.allow_origin(value),
            Err(err) => {
                tracing::warn!(origin = %origin, error = %err, "skipping malformed CORS origin");
            }
        }
    }
    layer
}

fn build_rate_limiter_config(
    config: &Config,
) -> Arc<
    tower_governor::governor::GovernorConfig<
        tower_governor::key_extractor::SmartIpKeyExtractor,
        governor::middleware::NoOpMiddleware,
    >,
> {
    let per_minute = config.rate_limit_per_minute.max(1);
    let per_second = (per_minute / 60).max(1);
    let burst = (per_minute / 4).max(1);

    // SmartIpKeyExtractor reads X-Forwarded-For / Forwarded / X-Real-IP
    // before falling back to peer IP. Behind Fly's edge proxy the peer
    // IP is ALWAYS the proxy itself, so PeerIpKeyExtractor would
    // collapse the entire fleet into a single bucket. Fly sets
    // X-Forwarded-For, which the smart extractor honours.
    //
    // `finish()` only returns `None` for impossible values (zero burst /
    // zero period). Both are clamped to ≥ 1 above, so the unwrap is safe.
    #[allow(clippy::expect_used)]
    let governor_conf = GovernorConfigBuilder::default()
        .per_second(u64::from(per_second))
        .burst_size(burst)
        .key_extractor(tower_governor::key_extractor::SmartIpKeyExtractor)
        .finish()
        .expect("rate limiter config is valid (per_second and burst clamped >= 1)");
    Arc::new(governor_conf)
}

/// Resolve the bind address from config.
pub fn bind_addr(config: &Config) -> SocketAddr {
    let host = config.app_host.parse().unwrap_or_else(|_| {
        tracing::warn!(host = %config.app_host, "APP_HOST is not a valid IP, defaulting to 0.0.0.0");
        std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)
    });
    SocketAddr::from((host, config.app_port))
}
