//! OAuth core types — Track J PR-J1 / Goal v3 §V Phase 10.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::OffsetDateTime;

/// Connected OAuth provider identifier. Lowercase serde so the JSON
/// shape matches the URL slug (`POST /v1/oauth/connect/google-drive`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Provider {
    /// Google Drive — read-only access to the Mizan-watched folder
    /// for bank statement ingestion.
    GoogleDrive,
    /// Notion — single Mizan-designated database for goals.
    Notion,
    /// Slack — Today's Signal delivery channel.
    Slack,
    /// GitHub — read-only equity-comp tracking via repository
    /// `mizan-equity-comp.json` files in the user's profile.
    Github,
    /// Apple Calendar — deadline surfacing for Zakat Hawl, tax
    /// quarterly estimates, etc.
    AppleCalendar,
    /// Google Calendar — same as Apple Calendar.
    GoogleCalendar,
    /// Outlook Calendar — same.
    OutlookCalendar,
    /// Zapier — automation bridge for users with existing zap
    /// catalogs.
    Zapier,
}

impl Provider {
    /// Parse a free-text provider slug. Returns None for unrecognised
    /// strings; the caller surfaces 404.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_lowercase().as_str() {
            "google-drive" | "google_drive" | "googledrive" => Some(Self::GoogleDrive),
            "notion" => Some(Self::Notion),
            "slack" => Some(Self::Slack),
            "github" => Some(Self::Github),
            "apple-calendar" | "apple_calendar" | "icloud-calendar" => Some(Self::AppleCalendar),
            "google-calendar" | "google_calendar" => Some(Self::GoogleCalendar),
            "outlook-calendar" | "outlook_calendar" | "microsoft-calendar" => {
                Some(Self::OutlookCalendar)
            }
            "zapier" => Some(Self::Zapier),
            _ => None,
        }
    }

    /// URL slug used in route paths.
    pub fn slug(self) -> &'static str {
        match self {
            Self::GoogleDrive => "google-drive",
            Self::Notion => "notion",
            Self::Slack => "slack",
            Self::Github => "github",
            Self::AppleCalendar => "apple-calendar",
            Self::GoogleCalendar => "google-calendar",
            Self::OutlookCalendar => "outlook-calendar",
            Self::Zapier => "zapier",
        }
    }

    /// Human-readable display name for the desktop UI.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::GoogleDrive => "Google Drive",
            Self::Notion => "Notion",
            Self::Slack => "Slack",
            Self::Github => "GitHub",
            Self::AppleCalendar => "Apple Calendar",
            Self::GoogleCalendar => "Google Calendar",
            Self::OutlookCalendar => "Outlook Calendar",
            Self::Zapier => "Zapier",
        }
    }
}

/// Per-provider scope enum. Each variant represents a single OAuth
/// scope string the provider's authorization server understands. The
/// catalog (in `catalog.rs`) maps each Provider to its required scope
/// list.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OAuthScopes {
    /// `https://www.googleapis.com/auth/drive.readonly`
    GoogleDriveReadonly,
    /// `https://www.googleapis.com/auth/calendar.events.readonly`
    GoogleCalendarEventsReadonly,
    /// Notion: full DB read/write (only the Mizan-designated DB; the
    /// integration scoping is enforced at app-install time, not via
    /// OAuth scopes).
    NotionRead,
    NotionWrite,
    /// Slack: post messages to channels the bot is invited to.
    SlackChatWrite,
    /// GitHub: read user repos + contents only. Never includes `write`
    /// or `admin`.
    GithubReadRepo,
    /// Outlook Calendar: read events.
    OutlookCalendarRead,
    /// Apple Calendar uses EventKit on-device — no OAuth scope. Sentinel
    /// variant for catalog consistency.
    AppleCalendarEventKit,
    /// Zapier: webhook URLs only — no token-exchange OAuth, but
    /// surfaced through the same catalog for symmetry.
    ZapierWebhook,
}

impl OAuthScopes {
    /// The raw scope string sent in the OAuth authorization request.
    pub fn raw_scope(&self) -> &'static str {
        match self {
            Self::GoogleDriveReadonly => "https://www.googleapis.com/auth/drive.readonly",
            Self::GoogleCalendarEventsReadonly => {
                "https://www.googleapis.com/auth/calendar.events.readonly"
            }
            Self::NotionRead => "read_content",
            Self::NotionWrite => "update_content",
            Self::SlackChatWrite => "chat:write",
            Self::GithubReadRepo => "repo:read",
            Self::OutlookCalendarRead => "Calendars.Read",
            Self::AppleCalendarEventKit => "eventkit-local",
            Self::ZapierWebhook => "webhook",
        }
    }
}

/// Builder-style input for `Provider`'s authorization URL. The flow:
///
/// 1. Desktop calls `POST /v1/oauth/connect/{provider}` (PR-J1.b).
/// 2. Mizan Connect builds an `AuthorizationRequest` with a fresh
///    HMAC-signed `state` param + the provider's scope list from the
///    catalog.
/// 3. The desktop opens the resulting URL in the system browser.
/// 4. User authenticates → provider redirects to the callback URL.
/// 5. PR-J1.b's callback handler exchanges the `code` for a `TokenSet`.
#[derive(Debug, Clone, Serialize)]
pub struct AuthorizationRequest {
    pub provider: Provider,
    pub authorization_url: String,
    /// HMAC-signed state param — verified at callback to prevent CSRF.
    pub state: String,
    /// Scopes the user is about to grant. Rendered on the consent
    /// screen via the provider's UI.
    pub scopes: Vec<&'static str>,
    /// Where the provider redirects after consent.
    pub redirect_uri: String,
}

/// Triple returned by the token exchange. Encrypted at rest via the
/// per-provider encryption key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSet {
    pub provider: Provider,
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Provider-reported expiry timestamp. The background refresh
    /// worker uses this to schedule pre-expiry refreshes.
    #[serde(with = "time::serde::rfc3339::option", default)]
    pub expires_at: Option<OffsetDateTime>,
    /// Token scope, comma-separated, as the provider echoed back.
    /// Useful for debugging "why am I getting 403 on this endpoint".
    pub granted_scopes: Vec<String>,
}

/// Errors raised by the OAuth flow handlers.
#[derive(Debug, Error)]
pub enum OAuthError {
    #[error("provider '{0}' is not in the catalog")]
    UnknownProvider(String),
    #[error("state mismatch — possible CSRF attempt")]
    StateMismatch,
    #[error("token exchange failed: {0}")]
    TokenExchangeFailed(String),
    #[error("refresh failed: {0}")]
    RefreshFailed(String),
    #[error("encryption key unavailable")]
    KeyUnavailable,
    #[error("provider returned no refresh token — re-auth required for long-lived access")]
    NoRefreshToken,
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn provider_parse_canonical_slugs() {
        assert_eq!(Provider::parse("google-drive"), Some(Provider::GoogleDrive));
        assert_eq!(Provider::parse("notion"), Some(Provider::Notion));
        assert_eq!(Provider::parse("slack"), Some(Provider::Slack));
        assert_eq!(Provider::parse("github"), Some(Provider::Github));
        assert_eq!(
            Provider::parse("apple-calendar"),
            Some(Provider::AppleCalendar)
        );
        assert_eq!(
            Provider::parse("google-calendar"),
            Some(Provider::GoogleCalendar)
        );
        assert_eq!(
            Provider::parse("outlook-calendar"),
            Some(Provider::OutlookCalendar)
        );
        assert_eq!(Provider::parse("zapier"), Some(Provider::Zapier));
    }

    #[test]
    fn provider_parse_aliases() {
        assert_eq!(Provider::parse("google_drive"), Some(Provider::GoogleDrive));
        assert_eq!(Provider::parse("googledrive"), Some(Provider::GoogleDrive));
        assert_eq!(
            Provider::parse("icloud-calendar"),
            Some(Provider::AppleCalendar)
        );
        assert_eq!(
            Provider::parse("microsoft-calendar"),
            Some(Provider::OutlookCalendar)
        );
        assert_eq!(Provider::parse(" NOTION "), Some(Provider::Notion));
    }

    #[test]
    fn provider_parse_rejects_unknown() {
        assert_eq!(Provider::parse("dropbox"), None);
        assert_eq!(Provider::parse(""), None);
        assert_eq!(Provider::parse("scam-provider"), None);
    }

    #[test]
    fn provider_slugs_round_trip_through_parse() {
        for provider in [
            Provider::GoogleDrive,
            Provider::Notion,
            Provider::Slack,
            Provider::Github,
            Provider::AppleCalendar,
            Provider::GoogleCalendar,
            Provider::OutlookCalendar,
            Provider::Zapier,
        ] {
            assert_eq!(Provider::parse(provider.slug()), Some(provider));
        }
    }

    #[test]
    fn display_names_are_human_readable() {
        assert_eq!(Provider::GoogleDrive.display_name(), "Google Drive");
        assert_eq!(Provider::Notion.display_name(), "Notion");
        assert_eq!(Provider::OutlookCalendar.display_name(), "Outlook Calendar");
    }

    #[test]
    fn scopes_have_canonical_provider_strings() {
        assert!(OAuthScopes::GoogleDriveReadonly
            .raw_scope()
            .contains("googleapis.com/auth/drive.readonly"));
        assert_eq!(OAuthScopes::SlackChatWrite.raw_scope(), "chat:write");
        assert_eq!(OAuthScopes::GithubReadRepo.raw_scope(), "repo:read");
    }

    #[test]
    fn provider_serde_kebab_case() {
        let json = serde_json::to_string(&Provider::GoogleDrive).expect("ok");
        assert_eq!(json, "\"google-drive\"");
        let parsed: Provider = serde_json::from_str("\"notion\"").expect("ok");
        assert_eq!(parsed, Provider::Notion);
    }
}
