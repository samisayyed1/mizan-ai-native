//! Notifications command schemas — worked example demonstrating the
//! v1/v2 pattern.
//!
//! See ADR 0010 for the architecture rationale.

use serde::{Deserialize, Serialize};

#[cfg(feature = "ts-export")]
use ts_rs::TS;

/// Version 1: the current shipping shape. Lists notifications with
/// pagination via cursor.
pub mod v1 {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[cfg_attr(feature = "ts-export", derive(TS))]
    #[cfg_attr(
        feature = "ts-export",
        ts(export, export_to = "../../../apps/frontend/src/lib/ipc-types/")
    )]
    pub struct NotificationsListRequest {
        pub limit: u32,
        pub cursor: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[cfg_attr(feature = "ts-export", derive(TS))]
    #[cfg_attr(
        feature = "ts-export",
        ts(export, export_to = "../../../apps/frontend/src/lib/ipc-types/")
    )]
    pub struct NotificationsListResponse {
        pub items: Vec<NotificationItem>,
        pub next_cursor: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[cfg_attr(feature = "ts-export", derive(TS))]
    #[cfg_attr(
        feature = "ts-export",
        ts(export, export_to = "../../../apps/frontend/src/lib/ipc-types/")
    )]
    pub struct NotificationItem {
        pub id: String,
        pub category: String,
        pub headline: String,
        pub preview: String,
        pub timestamp: String,
        pub is_read: bool,
    }
}

// When the panel polish in Track A PR-A12 lands and adds a `severity`
// field + `filter` request param, a `mod v2` lives here. The handler
// dispatches via the `try_parse_versions!` macro. Until then v1 is the
// only shipped version.

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::v1::*;

    #[test]
    fn request_serde_round_trip() {
        let req = NotificationsListRequest {
            limit: 25,
            cursor: Some("cursor-abc".into()),
        };
        let json = serde_json::to_value(&req).expect("serialise");
        let back: NotificationsListRequest = serde_json::from_value(json).expect("deserialise");
        assert_eq!(back.limit, req.limit);
        assert_eq!(back.cursor, req.cursor);
    }

    #[test]
    fn request_optional_cursor_omits_cleanly() {
        let req = NotificationsListRequest {
            limit: 25,
            cursor: None,
        };
        let json = serde_json::to_value(&req).expect("serialise");
        // None should serialise as JSON null per serde defaults
        assert!(json.get("cursor").is_some());
    }
}
