//! OAuth provider catalog — Track J PR-J1 / Goal v3 §V Phase 10.
//!
//! Hard-coded descriptors for each connected provider — analogous to
//! the `pay_zakat::CATALOG` discipline in mizan-zakat. The catalog is
//! the single source of truth for provider URLs + scope lists at
//! deploy time.

use serde::Serialize;

use super::types::{OAuthScopes, Provider};

/// Per-provider metadata used to build authorization URLs + render
/// the desktop UI's Connections list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDescriptor {
    pub provider: Provider,
    /// OAuth authorization endpoint.
    pub authorization_endpoint: &'static str,
    /// OAuth token-exchange endpoint.
    pub token_endpoint: &'static str,
    /// Optional revocation endpoint. None for providers that don't
    /// expose one (Zapier).
    pub revocation_endpoint: Option<&'static str>,
    /// Minimum scopes required for the integration's stated use case.
    pub required_scopes: &'static [OAuthScopes],
    /// Two-line description for the catalog UI.
    pub description: &'static str,
    /// Whether the provider issues refresh tokens. If false, the
    /// background refresh worker (PR-J1.c) skips it and the user
    /// re-auths on token expiry.
    pub issues_refresh_token: bool,
}

/// The vetted OAuth provider catalog. Per ADR 0025, adding entries
/// requires a code change + a Track J follow-up PR documenting the
/// integration's use case + scope justification.
pub const PROVIDERS: &[ProviderDescriptor] = &[
    ProviderDescriptor {
        provider: Provider::GoogleDrive,
        authorization_endpoint: "https://accounts.google.com/o/oauth2/v2/auth",
        token_endpoint: "https://oauth2.googleapis.com/token",
        revocation_endpoint: Some("https://oauth2.googleapis.com/revoke"),
        required_scopes: &[OAuthScopes::GoogleDriveReadonly],
        description: "Read statements + tax docs from a Mizan-watched folder. Read-only. \
             Mizan never lists or accesses any other Drive content.",
        issues_refresh_token: true,
    },
    ProviderDescriptor {
        provider: Provider::Notion,
        authorization_endpoint: "https://api.notion.com/v1/oauth/authorize",
        token_endpoint: "https://api.notion.com/v1/oauth/token",
        revocation_endpoint: None,
        required_scopes: &[OAuthScopes::NotionRead, OAuthScopes::NotionWrite],
        description: "Sync goals to a Mizan-designated Notion database. Read + write \
             access only on the chosen DB; nothing else in your workspace.",
        issues_refresh_token: false,
    },
    ProviderDescriptor {
        provider: Provider::Slack,
        authorization_endpoint: "https://slack.com/oauth/v2/authorize",
        token_endpoint: "https://slack.com/api/oauth.v2.access",
        revocation_endpoint: Some("https://slack.com/api/auth.revoke"),
        required_scopes: &[OAuthScopes::SlackChatWrite],
        description: "Post Today's Signal + Hawl reminders to a channel of your \
             choice. Posts only — never reads channel content.",
        issues_refresh_token: true,
    },
    ProviderDescriptor {
        provider: Provider::Github,
        authorization_endpoint: "https://github.com/login/oauth/authorize",
        token_endpoint: "https://github.com/login/oauth/access_token",
        revocation_endpoint: None,
        required_scopes: &[OAuthScopes::GithubReadRepo],
        description: "Pull equity-compensation snapshots from a `mizan-equity-comp.json` \
             file in your repos. Read-only access to repo contents only.",
        issues_refresh_token: false,
    },
    ProviderDescriptor {
        provider: Provider::GoogleCalendar,
        authorization_endpoint: "https://accounts.google.com/o/oauth2/v2/auth",
        token_endpoint: "https://oauth2.googleapis.com/token",
        revocation_endpoint: Some("https://oauth2.googleapis.com/revoke"),
        required_scopes: &[OAuthScopes::GoogleCalendarEventsReadonly],
        description: "Surface upcoming Zakat Hawl, tax estimate, and Sukuk maturity \
             dates as calendar events. Read-only.",
        issues_refresh_token: true,
    },
    ProviderDescriptor {
        provider: Provider::OutlookCalendar,
        authorization_endpoint: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
        token_endpoint: "https://login.microsoftonline.com/common/oauth2/v2.0/token",
        revocation_endpoint: None,
        required_scopes: &[OAuthScopes::OutlookCalendarRead],
        description: "Same as Google Calendar but for Microsoft 365 + outlook.com users.",
        issues_refresh_token: true,
    },
    ProviderDescriptor {
        provider: Provider::AppleCalendar,
        // Apple Calendar uses EventKit on-device. No remote
        // authorization endpoint — the desktop requests permission
        // via the macOS EventKit framework. Catalog entry exists for
        // symmetry; PR-J1.b's handler short-circuits with a
        // platform-native flow instead of an OAuth redirect.
        authorization_endpoint: "eventkit://local",
        token_endpoint: "eventkit://local",
        revocation_endpoint: None,
        required_scopes: &[OAuthScopes::AppleCalendarEventKit],
        description: "Surface upcoming Zakat Hawl, tax estimate, and Sukuk maturity \
             dates in Apple Calendar via macOS EventKit (on-device permission).",
        issues_refresh_token: false,
    },
    ProviderDescriptor {
        provider: Provider::Zapier,
        // Zapier connects via user-supplied webhook URLs, not OAuth.
        // PR-J1.b's connect handler stores the webhook URL directly
        // without going through an authorization redirect.
        authorization_endpoint: "https://zapier.com/app/connections",
        token_endpoint: "https://hooks.zapier.com/hooks/catch/",
        revocation_endpoint: None,
        required_scopes: &[OAuthScopes::ZapierWebhook],
        description: "Trigger your existing Zaps from Mizan events (Today's Signal, \
             Hawl reached, Zakat paid). Outbound webhooks only.",
        issues_refresh_token: false,
    },
];

/// Look up the descriptor for a given provider. Returns `None` for
/// providers not in the catalog (defensive — the caller surfaces
/// 404).
pub fn provider_descriptor(provider: Provider) -> Option<&'static ProviderDescriptor> {
    PROVIDERS.iter().find(|p| p.provider == provider)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn catalog_covers_all_8_directive_required_providers() {
        // Per Mizan_Continue_Autonomous_v3.md line 77:
        // "Google Drive, Notion, Slack, GitHub, Apple/Google/Outlook
        //  Calendar, Zapier" — 8 providers total.
        assert_eq!(PROVIDERS.len(), 8);
    }

    #[test]
    fn catalog_includes_each_provider_variant() {
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
            assert!(
                provider_descriptor(provider).is_some(),
                "{provider:?} missing from catalog"
            );
        }
    }

    #[test]
    fn every_descriptor_has_non_empty_endpoints() {
        for d in PROVIDERS {
            assert!(
                !d.authorization_endpoint.is_empty(),
                "{:?} missing authorization_endpoint",
                d.provider
            );
            assert!(
                !d.token_endpoint.is_empty(),
                "{:?} missing token_endpoint",
                d.provider
            );
            assert!(
                !d.description.is_empty(),
                "{:?} missing description",
                d.provider
            );
        }
    }

    #[test]
    fn every_descriptor_has_at_least_one_required_scope() {
        for d in PROVIDERS {
            assert!(
                !d.required_scopes.is_empty(),
                "{:?} declares no required_scopes — security boundary undefined",
                d.provider
            );
        }
    }

    #[test]
    fn refresh_token_providers_are_correctly_flagged() {
        // Per the OAuth spec + provider docs:
        // - Google issues refresh tokens (with access_type=offline)
        // - Slack issues refresh tokens since v2 OAuth
        // - Outlook issues refresh tokens
        // - Notion doesn't issue refresh tokens (long-lived
        //   integration tokens)
        // - GitHub doesn't issue refresh tokens (long-lived user
        //   access tokens)
        // - Apple Calendar / Zapier aren't real OAuth flows
        assert!(
            provider_descriptor(Provider::GoogleDrive)
                .unwrap()
                .issues_refresh_token
        );
        assert!(
            provider_descriptor(Provider::GoogleCalendar)
                .unwrap()
                .issues_refresh_token
        );
        assert!(
            provider_descriptor(Provider::Slack)
                .unwrap()
                .issues_refresh_token
        );
        assert!(
            provider_descriptor(Provider::OutlookCalendar)
                .unwrap()
                .issues_refresh_token
        );
        assert!(
            !provider_descriptor(Provider::Notion)
                .unwrap()
                .issues_refresh_token
        );
        assert!(
            !provider_descriptor(Provider::Github)
                .unwrap()
                .issues_refresh_token
        );
        assert!(
            !provider_descriptor(Provider::AppleCalendar)
                .unwrap()
                .issues_refresh_token
        );
        assert!(
            !provider_descriptor(Provider::Zapier)
                .unwrap()
                .issues_refresh_token
        );
    }

    #[test]
    fn google_drive_uses_readonly_scope() {
        let d = provider_descriptor(Provider::GoogleDrive).unwrap();
        assert_eq!(d.required_scopes.len(), 1);
        assert!(d.required_scopes[0].raw_scope().contains("readonly"));
    }

    #[test]
    fn github_uses_read_only_scope_never_admin() {
        let d = provider_descriptor(Provider::Github).unwrap();
        let scope = d.required_scopes[0].raw_scope();
        assert!(scope.contains("read"));
        assert!(!scope.contains("admin"));
        assert!(!scope.contains("write"));
    }

    #[test]
    fn descriptor_serde_roundtrip() {
        let d = provider_descriptor(Provider::GoogleDrive).unwrap();
        let json = serde_json::to_string(&d).expect("ok");
        assert!(json.contains("googleapis.com"));
        assert!(json.contains("google-drive"));
    }
}
