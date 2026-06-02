//! Vite content-hash bundle verification (Track I PR-I8.a per ADR 0017).
//!
//! Ships the verifier's API + tested helpers without yet wiring the
//! production embedding (that's PR-I8.b). The integration point in
//! `lib.rs::setup` is reserved as a no-op call site so PR-I8.b only
//! needs to swap the implementation.
//!
//! See [ADR 0017](../../../docs/adr/0017-vite-content-hash-bundle-verification.md)
//! for full design rationale + the I8.a vs I8.b split.
//!
//! # On `#[allow(dead_code)]`
//!
//! Every type below is exercised by the unit-test fixtures + by
//! `verify_embedded_bundle`'s scaffold stub. The dead-code allow on the
//! module covers the surface that PR-I8.b activates from the tauri-build
//! hook: production callers (the build-time manifest emitter + the
//! runtime fetcher backed by the embedded bundle) don't exist yet. The
//! allowance is removed in PR-I8.b alongside those callers.

#![allow(dead_code)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One asset's expected content hash. Vite emits content-hashed
/// filenames (`assets/index-A1B2C3.js`); we keep both so a missing
/// asset and a hash mismatch produce distinct error messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEntry {
    /// Path within the bundle root (e.g. `assets/index-A1B2C3.js`).
    pub path: String,
    /// blake3 hex-encoded content hash. Computed at build time by the
    /// tauri-build hook (PR-I8.b). Lower-case hex per blake3 convention.
    pub blake3_hex: String,
}

/// The manifest produced by the build-system hook and embedded into
/// the binary. Wraps a BTreeMap so the iteration order is deterministic
/// (helps log output be greppable + tests stay stable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Manifest {
    pub entries: BTreeMap<String, ManifestEntry>,
}

impl Manifest {
    /// Parse a manifest from a JSON byte slice (typically `include_bytes!`
    /// output in PR-I8.b).
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, BundleError> {
        serde_json::from_slice(bytes).map_err(|e| BundleError::ManifestParse(e.to_string()))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Abstract source of asset bytes. Production impl reads from the
/// embedded Tauri bundle; tests pass an in-memory map.
///
/// Keeping this as a trait lets the unit tests exercise the verifier
/// without standing up a Tauri runtime.
pub trait FetchAsset {
    /// Return the bytes for `path`, or None if not found in the bundle.
    /// Implementations should treat "not found" and "I/O error" the
    /// same way — the caller surfaces `AssetMissing` either way.
    fn fetch(&self, path: &str) -> Option<Vec<u8>>;
}

/// Aggregate outcome of a verification run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BundleVerification {
    /// All N entries in the manifest matched the bundle's actual
    /// content. The desktop continues startup normally.
    Clean { entries_verified: usize },
    /// Some entries were missing OR mismatched. The caller (PR-I8.b's
    /// integration point) wipes the WebView cache + reloads.
    Mismatched {
        missing: Vec<String>,
        mismatched: Vec<MismatchDetail>,
    },
    /// The manifest itself was absent (e.g. PR-I8.a scaffold mode
    /// before the build hook lands, or a dev-mode build without the
    /// manifest emit). Treated as informational; PR-I8.b promotes
    /// this to `Err(ManifestMissing)`.
    Skipped { reason: String },
}

/// Detail of one entry whose blake3 didn't match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MismatchDetail {
    pub path: String,
    pub expected_blake3_hex: String,
    pub actual_blake3_hex: String,
}

/// Errors from the verifier. Catastrophic conditions only — per-entry
/// failures become a `Mismatched` outcome, not an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BundleError {
    /// JSON parsing failed at the manifest level (corrupted, not just
    /// a missing field).
    ManifestParse(String),
}

impl std::fmt::Display for BundleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ManifestParse(msg) => write!(f, "manifest parse failed: {msg}"),
        }
    }
}

impl std::error::Error for BundleError {}

/// Verify a manifest against a [`FetchAsset`] source. Pure function —
/// no I/O of its own beyond what the `FetchAsset` impl performs.
///
/// Returns `BundleVerification::Clean` iff every entry exists and its
/// blake3 matches the expected value. Returns `Mismatched` otherwise.
pub fn verify_against_manifest(
    manifest: &Manifest,
    fetcher: &dyn FetchAsset,
) -> BundleVerification {
    let mut missing = Vec::new();
    let mut mismatched = Vec::new();

    for (path, entry) in &manifest.entries {
        let Some(bytes) = fetcher.fetch(path) else {
            missing.push(path.clone());
            continue;
        };
        let actual = blake3::hash(&bytes).to_hex().to_string();
        if actual != entry.blake3_hex.to_lowercase() {
            mismatched.push(MismatchDetail {
                path: path.clone(),
                expected_blake3_hex: entry.blake3_hex.clone(),
                actual_blake3_hex: actual,
            });
        }
    }

    if missing.is_empty() && mismatched.is_empty() {
        BundleVerification::Clean {
            entries_verified: manifest.entries.len(),
        }
    } else {
        BundleVerification::Mismatched {
            missing,
            mismatched,
        }
    }
}

/// PR-I8.a scaffold entry point. The production call site (PR-I8.b)
/// replaces this with a manifest derived from `include_bytes!` of the
/// embedded manifest.json.
///
/// In I8.a we return `Skipped` so the no-op integration point in
/// lib.rs::setup can log the scaffold marker without disrupting startup.
pub fn verify_embedded_bundle() -> Result<BundleVerification, BundleError> {
    Ok(BundleVerification::Skipped {
        reason: "PR-I8.a scaffold — embedding hook lands in PR-I8.b".to_string(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    /// In-memory fetcher fixture.
    struct MemFetcher(BTreeMap<String, Vec<u8>>);

    impl FetchAsset for MemFetcher {
        fn fetch(&self, path: &str) -> Option<Vec<u8>> {
            self.0.get(path).cloned()
        }
    }

    fn entry(path: &str, blake3_hex: &str) -> (String, ManifestEntry) {
        (
            path.to_string(),
            ManifestEntry {
                path: path.to_string(),
                blake3_hex: blake3_hex.to_string(),
            },
        )
    }

    #[test]
    fn clean_when_every_entry_matches() {
        let bytes_index = b"hello-world";
        let h_index = blake3::hash(bytes_index).to_hex().to_string();
        let manifest = Manifest {
            entries: [entry("assets/index-A1.js", &h_index)].into(),
        };
        let fetcher = MemFetcher([("assets/index-A1.js".into(), bytes_index.to_vec())].into());

        let out = verify_against_manifest(&manifest, &fetcher);
        assert_eq!(
            out,
            BundleVerification::Clean {
                entries_verified: 1
            }
        );
    }

    #[test]
    fn mismatched_when_content_differs() {
        let h_expected = blake3::hash(b"expected").to_hex().to_string();
        let manifest = Manifest {
            entries: [entry("assets/x.js", &h_expected)].into(),
        };
        let fetcher = MemFetcher([("assets/x.js".to_string(), b"actual".to_vec())].into());

        match verify_against_manifest(&manifest, &fetcher) {
            BundleVerification::Mismatched {
                missing,
                mismatched,
            } => {
                assert!(missing.is_empty());
                assert_eq!(mismatched.len(), 1);
                assert_eq!(mismatched[0].path, "assets/x.js");
                assert_eq!(mismatched[0].expected_blake3_hex, h_expected);
            }
            other => panic!("expected Mismatched, got {other:?}"),
        }
    }

    #[test]
    fn missing_when_fetcher_returns_none() {
        let h = blake3::hash(b"x").to_hex().to_string();
        let manifest = Manifest {
            entries: [entry("assets/gone.js", &h)].into(),
        };
        let fetcher = MemFetcher(BTreeMap::new());

        match verify_against_manifest(&manifest, &fetcher) {
            BundleVerification::Mismatched {
                missing,
                mismatched,
            } => {
                assert_eq!(missing, vec!["assets/gone.js"]);
                assert!(mismatched.is_empty());
            }
            other => panic!("expected Mismatched, got {other:?}"),
        }
    }

    #[test]
    fn empty_manifest_is_clean_zero_entries() {
        let manifest = Manifest::default();
        let fetcher = MemFetcher(BTreeMap::new());
        let out = verify_against_manifest(&manifest, &fetcher);
        assert_eq!(
            out,
            BundleVerification::Clean {
                entries_verified: 0
            }
        );
    }

    #[test]
    fn manifest_parses_well_formed_json() {
        let json = br#"{
            "entries": {
                "assets/index-A1.js": {
                    "path": "assets/index-A1.js",
                    "blake3_hex": "deadbeef"
                }
            }
        }"#;
        let m = Manifest::from_json_bytes(json).unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m.entries["assets/index-A1.js"].blake3_hex, "deadbeef");
    }

    #[test]
    fn manifest_parse_returns_err_on_garbage() {
        let json = b"not json at all";
        let err = Manifest::from_json_bytes(json).unwrap_err();
        assert!(matches!(err, BundleError::ManifestParse(_)));
    }

    #[test]
    fn verify_embedded_bundle_returns_skipped_in_scaffold() {
        let out = verify_embedded_bundle().unwrap();
        match out {
            BundleVerification::Skipped { reason } => {
                assert!(reason.contains("PR-I8.a"));
            }
            other => panic!("expected Skipped, got {other:?}"),
        }
    }

    #[test]
    fn mixed_missing_and_mismatched_reports_both() {
        let h_present = blake3::hash(b"present-bytes").to_hex().to_string();
        let h_expected_for_other = blake3::hash(b"expected-other").to_hex().to_string();
        let manifest = Manifest {
            entries: [
                entry("assets/present.js", &h_present),
                entry("assets/missing.js", "irrelevant-hash"),
                entry("assets/wrong.js", &h_expected_for_other),
            ]
            .into(),
        };
        let fetcher = MemFetcher(
            [
                ("assets/present.js".to_string(), b"present-bytes".to_vec()),
                (
                    "assets/wrong.js".to_string(),
                    b"completely-different".to_vec(),
                ),
            ]
            .into(),
        );

        match verify_against_manifest(&manifest, &fetcher) {
            BundleVerification::Mismatched {
                missing,
                mismatched,
            } => {
                assert_eq!(missing, vec!["assets/missing.js"]);
                assert_eq!(mismatched.len(), 1);
                assert_eq!(mismatched[0].path, "assets/wrong.js");
            }
            other => panic!("expected Mismatched, got {other:?}"),
        }
    }
}
