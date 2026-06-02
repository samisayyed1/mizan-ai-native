//! `X-Mizan-Client-Version` middleware.
//!
//! Reads the client version header from incoming requests and exposes it
//! to handlers via a tokio task-local. Handlers that need to branch by
//! desktop client version (e.g. during a migration window where a
//! breaking API change ships in vN+1) call [`ClientVersion::current`] to
//! get the parsed semver.
//!
//! # Why a middleware rather than a path-prefix
//!
//! Path-prefix versioning (`/v1/`, `/v2/`) per ADR 0013 handles the
//! cross-version API surface. This middleware handles the
//! intra-version-major desktop binary version — useful for telemetry
//! (per-version request counts on the admin dashboard) and for
//! handlers that want to branch behaviour on a specific client patch
//! release (a temporary workaround for a desktop bug, etc.).
//!
//! # Format
//!
//! The header is parsed as semver per [semver.org](https://semver.org/).
//! Pre-release + build-metadata are allowed. Invalid headers are
//! silently dropped (the request still serves; the value just isn't
//! available to handlers). Rejecting outright would break clients on
//! odd CI builds.
//!
//! # Min-version policy
//!
//! NOT enforced in this middleware. A future PR layers a
//! `MinClientVersionPolicy` on top using the [`semver::VersionReq`]
//! type if/when we need to refuse stale clients. For now this is
//! purely informational.

use std::task::{Context, Poll};

use axum::body::Body;
use axum::http::{HeaderName, Request, Response};
use futures::future::BoxFuture;
use semver::Version;
use tower::{Layer, Service};
use tracing::{field, Span};

pub const HEADER_NAME: HeaderName = HeaderName::from_static("x-mizan-client-version");
pub const TRACING_FIELD: &str = "client_version";

/// Maximum accepted header length. Real semver values cap out around
/// 30 chars including pre-release tags; 64 is a safe upper bound.
const MAX_LEN: usize = 64;

/// Parsed client version pulled from the request.
#[derive(Debug, Clone)]
pub struct ClientVersion(Version);

impl ClientVersion {
    pub fn version(&self) -> &Version {
        &self.0
    }

    /// Read the parsed version from the current task-local context, if
    /// the middleware ran and the header parsed successfully.
    pub fn current() -> Option<Self> {
        let mut out: Option<Version> = None;
        CLIENT_VERSION.try_with(|v| out = v.clone()).ok();
        out.map(ClientVersion)
    }
}

impl std::fmt::Display for ClientVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

tokio::task_local! {
    /// Set to `Some(version)` when the request carried a parseable
    /// `X-Mizan-Client-Version` header; `None` otherwise (older clients,
    /// CI smoke tests, etc.).
    static CLIENT_VERSION: Option<Version>;
}

/// Tower [`Layer`] producing [`ClientVersionService`].
#[derive(Clone, Default)]
pub struct ClientVersionLayer;

impl<S> Layer<S> for ClientVersionLayer {
    type Service = ClientVersionService<S>;
    fn layer(&self, inner: S) -> Self::Service {
        ClientVersionService { inner }
    }
}

/// Wrapping service that parses the version header and exposes it via
/// the task-local for downstream handlers.
#[derive(Clone)]
pub struct ClientVersionService<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for ClientVersionService<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let parsed = parse_header(&req);

        // Log the version (or "absent") on the tracing span so the
        // admin dashboard's per-version request panel can compute counts.
        match &parsed {
            Some(v) => Span::current().record(TRACING_FIELD, field::display(v)),
            None => Span::current().record(TRACING_FIELD, field::display("absent")),
        };

        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        Box::pin(async move {
            CLIENT_VERSION
                .scope(parsed, async move { inner.call(req).await })
                .await
        })
    }
}

fn parse_header<B>(req: &Request<B>) -> Option<Version> {
    req.headers()
        .get(HEADER_NAME)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.len() <= MAX_LEN)
        .and_then(|s| Version::parse(s).ok())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use axum::{body::Body, http::Request, routing::get, Router};
    use tower::ServiceExt;

    async fn handler() -> String {
        match ClientVersion::current() {
            Some(v) => format!("client={}", v.version()),
            None => "client=absent".to_string(),
        }
    }

    fn app() -> Router {
        Router::new()
            .route("/", get(handler))
            .layer(ClientVersionLayer)
    }

    async fn body_text(resp: axum::http::Response<Body>) -> String {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[tokio::test]
    async fn header_absent_returns_none() {
        let resp = app()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(body_text(resp).await, "client=absent");
    }

    #[tokio::test]
    async fn valid_semver_parses() {
        let req = Request::builder()
            .uri("/")
            .header(HEADER_NAME, "3.4.1")
            .body(Body::empty())
            .unwrap();
        let resp = app().oneshot(req).await.unwrap();
        assert_eq!(body_text(resp).await, "client=3.4.1");
    }

    #[tokio::test]
    async fn pre_release_parses() {
        let req = Request::builder()
            .uri("/")
            .header(HEADER_NAME, "3.4.1-beta.2")
            .body(Body::empty())
            .unwrap();
        let resp = app().oneshot(req).await.unwrap();
        assert_eq!(body_text(resp).await, "client=3.4.1-beta.2");
    }

    #[tokio::test]
    async fn build_metadata_parses() {
        let req = Request::builder()
            .uri("/")
            .header(HEADER_NAME, "3.4.1+sha.abc123")
            .body(Body::empty())
            .unwrap();
        let resp = app().oneshot(req).await.unwrap();
        assert_eq!(body_text(resp).await, "client=3.4.1+sha.abc123");
    }

    #[tokio::test]
    async fn garbage_silently_dropped() {
        // Per the docstring: invalid headers are silently dropped so
        // requests still serve. Rejecting outright would break clients
        // on odd CI builds.
        let req = Request::builder()
            .uri("/")
            .header(HEADER_NAME, "not-a-version")
            .body(Body::empty())
            .unwrap();
        let resp = app().oneshot(req).await.unwrap();
        assert_eq!(body_text(resp).await, "client=absent");
    }

    #[tokio::test]
    async fn over_max_len_dropped() {
        let oversized = "9".repeat(MAX_LEN + 1);
        let req = Request::builder()
            .uri("/")
            .header(HEADER_NAME, oversized)
            .body(Body::empty())
            .unwrap();
        let resp = app().oneshot(req).await.unwrap();
        assert_eq!(body_text(resp).await, "client=absent");
    }
}
