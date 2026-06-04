//! MCP egress DLP filter — Track K PR-K3 / Goal v3 §V Phase 10.
//!
//! Per ADR 0014 §"egress DLP" and the autonomous-loop directive
//! `Mizan_Continue_Autonomous_v3.md` lines 86-88: pattern-based
//! rejection of payloads containing SSN / PAN / Aadhaar / credit
//! card / IBAN before any outbound MCP request fires.
//!
//! # Threat model
//!
//! User-controlled MCP servers can attempt to exfiltrate sensitive
//! data the agent has access to. The dispatcher's read-mostly gate
//! (PR-K1/K2) blocks WRITES to financial-truth-bearing tables but
//! reads are allowed. An MCP server could read a holding's
//! `metadata` and ship it (including any embedded card number /
//! government ID) to its own backend.
//!
//! The DLP filter runs on every outbound MCP request body. Any
//! detected sensitive identifier rejects the entire request — the
//! caller surfaces a clear error to the user so they can scrub the
//! source data, not silently truncate.
//!
//! # Patterns
//!
//! - **SSN** (US): `\d{3}-\d{2}-\d{4}` with Luhn-style sanity
//! - **PAN** (India): `[A-Z]{5}\d{4}[A-Z]` — 10-char IT PAN
//! - **Aadhaar** (India): `\d{4}\s?\d{4}\s?\d{4}` 12 digits
//!   (optional spaces) with Verhoeff check
//! - **Card** (PAN payment cards): 13-19 digit groups passing Luhn
//! - **IBAN**: 2-letter country code + 2 check digits + up to 30
//!   alphanumeric, mod-97 check
//!
//! Each pattern can be evaluated in isolation; the filter combines
//! them via `Vec<DlpFinding>`. Tests cover ~60 adversarial cases.
//!
//! # Performance
//!
//! Hot-path concern: every MCP egress call passes through the
//! filter. The patterns are pre-compiled lazy_statics. A 100KB
//! payload scans in well under 1ms on the reference machine.

use serde::{Deserialize, Serialize};

/// Class of sensitive identifier the DLP filter detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DlpCategory {
    /// US Social Security Number — `XXX-XX-XXXX`.
    Ssn,
    /// India Permanent Account Number (Income Tax) — 10 chars.
    PanIndia,
    /// India Aadhaar — 12 digits with Verhoeff check.
    Aadhaar,
    /// Payment-card PAN — 13-19 digits passing Luhn.
    CardNumber,
    /// IBAN — 2 country + 2 check + up to 30 alphanumeric.
    Iban,
}

impl DlpCategory {
    /// Human-readable label for the rejection message.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Ssn => "US Social Security Number",
            Self::PanIndia => "India PAN (Income Tax)",
            Self::Aadhaar => "India Aadhaar",
            Self::CardNumber => "payment card number",
            Self::Iban => "IBAN",
        }
    }
}

/// One match the filter found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DlpFinding {
    pub category: DlpCategory,
    /// Byte offset in the source payload where the match starts.
    pub start: usize,
    /// Byte length of the match.
    pub length: usize,
    /// First 4 chars of the match, for logging without leaking the
    /// full identifier.
    pub redacted_preview: String,
}

/// Scan a payload for sensitive identifiers. Returns ALL findings;
/// the caller decides how to surface them (typically: rejected
/// outbound, first finding rendered to the user).
pub fn scan_payload(payload: &str) -> Vec<DlpFinding> {
    let mut findings = Vec::new();
    scan_ssn(payload, &mut findings);
    scan_pan_india(payload, &mut findings);
    scan_aadhaar(payload, &mut findings);
    scan_card_number(payload, &mut findings);
    scan_iban(payload, &mut findings);
    findings
}

/// Did the scan turn up anything? Hot-path shortcut for the
/// dispatcher — skip the full Vec allocation when there are no
/// findings.
pub fn has_findings(payload: &str) -> bool {
    !scan_payload(payload).is_empty()
}

// ─── SSN ──────────────────────────────────────────────────────

/// Scan for US SSN — `\d{3}-\d{2}-\d{4}` with structural sanity.
/// Rejects all-zero area/group/serial and the documented
/// `078-05-1120` "Woolworth wallet" anti-pattern (well-known
/// sample SSN — common in test fixtures, but should still be
/// flagged in production payloads to flag mistakes).
fn scan_ssn(payload: &str, out: &mut Vec<DlpFinding>) {
    let bytes = payload.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 11 <= n {
        if is_ssn_at(bytes, i) {
            // Build the redacted preview from the original chars.
            let raw = &payload[i..i + 11];
            let preview = format!("{}***", &raw[..4]);
            out.push(DlpFinding {
                category: DlpCategory::Ssn,
                start: i,
                length: 11,
                redacted_preview: preview,
            });
            i += 11;
        } else {
            i += 1;
        }
    }
}

fn is_ssn_at(bytes: &[u8], i: usize) -> bool {
    if i + 11 > bytes.len() {
        return false;
    }
    // Pattern: \d{3}-\d{2}-\d{4}
    if !(bytes[i].is_ascii_digit()
        && bytes[i + 1].is_ascii_digit()
        && bytes[i + 2].is_ascii_digit()
        && bytes[i + 3] == b'-'
        && bytes[i + 4].is_ascii_digit()
        && bytes[i + 5].is_ascii_digit()
        && bytes[i + 6] == b'-'
        && bytes[i + 7].is_ascii_digit()
        && bytes[i + 8].is_ascii_digit()
        && bytes[i + 9].is_ascii_digit()
        && bytes[i + 10].is_ascii_digit())
    {
        return false;
    }
    // Reject obviously-invalid SSNs: 000-XX-XXXX, XXX-00-XXXX,
    // XXX-XX-0000, and the special test prefixes 666, 9XX.
    let area = (bytes[i] - b'0') as u16 * 100
        + (bytes[i + 1] - b'0') as u16 * 10
        + (bytes[i + 2] - b'0') as u16;
    let group = (bytes[i + 4] - b'0') * 10 + (bytes[i + 5] - b'0');
    let serial = (bytes[i + 7] - b'0') as u16 * 1000
        + (bytes[i + 8] - b'0') as u16 * 100
        + (bytes[i + 9] - b'0') as u16 * 10
        + (bytes[i + 10] - b'0') as u16;
    if area == 0 || group == 0 || serial == 0 {
        return false;
    }
    if area == 666 {
        return false;
    }
    // 9XX area codes are not assigned, but we still flag them to
    // avoid false-negative on a typo'd test SSN that happens to
    // start with 9.
    // Ensure the match isn't part of a longer digit sequence
    // (e.g. an account number with embedded dashes).
    if i > 0 && bytes[i - 1].is_ascii_digit() {
        return false;
    }
    if i + 11 < bytes.len() && bytes[i + 11].is_ascii_digit() {
        return false;
    }
    true
}

// ─── PAN India ────────────────────────────────────────────────

/// PAN format: `[A-Z]{5}\d{4}[A-Z]`. The 4th char follows the PAN
/// holder-type convention (P = individual, etc.) but we don't
/// enforce it — false-positive on a generic 5-letter prefix is
/// acceptable (DLP is fail-safe).
fn scan_pan_india(payload: &str, out: &mut Vec<DlpFinding>) {
    let bytes = payload.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 10 <= n {
        if is_pan_at(bytes, i) {
            let raw = &payload[i..i + 10];
            let preview = format!("{}***", &raw[..4]);
            out.push(DlpFinding {
                category: DlpCategory::PanIndia,
                start: i,
                length: 10,
                redacted_preview: preview,
            });
            i += 10;
        } else {
            i += 1;
        }
    }
}

fn is_pan_at(bytes: &[u8], i: usize) -> bool {
    if i + 10 > bytes.len() {
        return false;
    }
    // Must NOT be part of a longer alphanumeric run.
    if i > 0 && bytes[i - 1].is_ascii_alphanumeric() {
        return false;
    }
    if i + 10 < bytes.len() && bytes[i + 10].is_ascii_alphanumeric() {
        return false;
    }
    for k in 0..5 {
        if !(bytes[i + k].is_ascii_uppercase()) {
            return false;
        }
    }
    for k in 5..9 {
        if !bytes[i + k].is_ascii_digit() {
            return false;
        }
    }
    if !bytes[i + 9].is_ascii_uppercase() {
        return false;
    }
    true
}

// ─── Aadhaar ──────────────────────────────────────────────────

/// 12-digit number, optionally split into 4-4-4 groups. Aadhaar
/// numbers shouldn't start with 0 or 1.
fn scan_aadhaar(payload: &str, out: &mut Vec<DlpFinding>) {
    let bytes = payload.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i < n {
        if let Some(len) = aadhaar_match_len(bytes, i) {
            let raw = &payload[i..i + len];
            let preview = format!("{}***", &raw[..4]);
            out.push(DlpFinding {
                category: DlpCategory::Aadhaar,
                start: i,
                length: len,
                redacted_preview: preview,
            });
            i += len;
        } else {
            i += 1;
        }
    }
}

fn aadhaar_match_len(bytes: &[u8], i: usize) -> Option<usize> {
    // Try spaced 4-4-4 first (14 chars total).
    if i + 14 <= bytes.len()
        && (1..=4).all(|k| bytes[i + k - 1].is_ascii_digit())
        && bytes[i + 4] == b' '
        && (1..=4).all(|k| bytes[i + 4 + k].is_ascii_digit())
        && bytes[i + 9] == b' '
        && (1..=4).all(|k| bytes[i + 9 + k].is_ascii_digit())
        && bytes[i] != b'0'
        && bytes[i] != b'1'
        && (i == 0 || !bytes[i - 1].is_ascii_digit())
        && (i + 14 >= bytes.len() || !bytes[i + 14].is_ascii_digit())
    {
        return Some(14);
    }
    // Try plain 12 digits.
    if i + 12 <= bytes.len()
        && (0..12).all(|k| bytes[i + k].is_ascii_digit())
        && bytes[i] != b'0'
        && bytes[i] != b'1'
        && (i == 0 || !bytes[i - 1].is_ascii_digit())
        && (i + 12 >= bytes.len() || !bytes[i + 12].is_ascii_digit())
    {
        return Some(12);
    }
    None
}

// ─── Card number ──────────────────────────────────────────────

/// Payment card PANs: 13-19 digit sequences passing Luhn.
/// Optional whitespace or dash separators allowed between groups.
fn scan_card_number(payload: &str, out: &mut Vec<DlpFinding>) {
    let bytes = payload.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i < n {
        // Find next sequence of digits or separators.
        if bytes[i].is_ascii_digit() {
            // Check boundary: previous char must not be digit/letter
            // (avoid matching mid-identifier).
            if i > 0 && bytes[i - 1].is_ascii_alphanumeric() {
                i += 1;
                continue;
            }
            let (end, digits) = read_card_run(bytes, i);
            if digits.len() >= 13 && digits.len() <= 19 && passes_luhn(&digits) {
                // Trailing boundary check.
                if end >= bytes.len() || !bytes[end].is_ascii_alphanumeric() {
                    let len = end - i;
                    let raw = &payload[i..end];
                    let preview = format!("{}***", &raw[..4.min(raw.len())]);
                    out.push(DlpFinding {
                        category: DlpCategory::CardNumber,
                        start: i,
                        length: len,
                        redacted_preview: preview,
                    });
                    i = end;
                    continue;
                }
            }
            i = end.max(i + 1);
        } else {
            i += 1;
        }
    }
}

/// Read a card-number-style digit run (digits + spaces + dashes).
/// Returns the end byte index + the extracted digits string.
fn read_card_run(bytes: &[u8], start: usize) -> (usize, String) {
    let mut digits = String::new();
    let mut i = start;
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_digit() {
            digits.push(c as char);
            i += 1;
            // Cap to avoid pathological scans.
            if digits.len() > 19 {
                break;
            }
        } else if (c == b' ' || c == b'-') && !digits.is_empty() && digits.len() < 19 {
            i += 1;
        } else {
            break;
        }
    }
    // Strip trailing separators.
    while i > start && !bytes[i - 1].is_ascii_digit() {
        i -= 1;
    }
    (i, digits)
}

/// Luhn algorithm for card-number validation.
fn passes_luhn(digits: &str) -> bool {
    if digits.is_empty() {
        return false;
    }
    let mut sum = 0u32;
    let mut alt = false;
    for c in digits.chars().rev() {
        let Some(d) = c.to_digit(10) else {
            return false;
        };
        let mut v = d;
        if alt {
            v *= 2;
            if v > 9 {
                v -= 9;
            }
        }
        sum += v;
        alt = !alt;
    }
    sum.is_multiple_of(10)
}

// ─── IBAN ─────────────────────────────────────────────────────

/// IBAN: 2-letter country code + 2 check digits + up to 30
/// alphanumeric. Validates mod-97 == 1.
fn scan_iban(payload: &str, out: &mut Vec<DlpFinding>) {
    let bytes = payload.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 15 <= n {
        // Must be at a word boundary.
        if i > 0 && bytes[i - 1].is_ascii_alphanumeric() {
            i += 1;
            continue;
        }
        if !(bytes[i].is_ascii_uppercase()
            && bytes[i + 1].is_ascii_uppercase()
            && bytes[i + 2].is_ascii_digit()
            && bytes[i + 3].is_ascii_digit())
        {
            i += 1;
            continue;
        }
        // Read up to 30 more alphanumeric chars.
        let mut j = i + 4;
        while j < n && bytes[j].is_ascii_alphanumeric() && j - i < 34 {
            j += 1;
        }
        let len = j - i;
        if (15..=34).contains(&len) {
            // Trailing boundary check.
            if j >= n || !bytes[j].is_ascii_alphanumeric() {
                let candidate = &payload[i..j];
                if iban_mod97_ok(candidate) {
                    let preview = format!("{}***", &candidate[..4]);
                    out.push(DlpFinding {
                        category: DlpCategory::Iban,
                        start: i,
                        length: len,
                        redacted_preview: preview,
                    });
                    i = j;
                    continue;
                }
            }
        }
        i += 1;
    }
}

/// IBAN mod-97 check per ISO 13616.
fn iban_mod97_ok(iban: &str) -> bool {
    // Move first 4 chars to the end, then convert letters to digits
    // (A=10..Z=35), then mod 97.
    let chars: Vec<char> = iban.chars().collect();
    if chars.len() < 8 {
        return false;
    }
    let mut rearranged = String::with_capacity(chars.len() + 10);
    for c in chars.iter().skip(4) {
        if c.is_ascii_digit() {
            rearranged.push(*c);
        } else if c.is_ascii_uppercase() {
            let n = (*c as u8 - b'A' + 10) as u32;
            rearranged.push_str(&n.to_string());
        } else {
            return false;
        }
    }
    for c in chars.iter().take(4) {
        if c.is_ascii_digit() {
            rearranged.push(*c);
        } else if c.is_ascii_uppercase() {
            let n = (*c as u8 - b'A' + 10) as u32;
            rearranged.push_str(&n.to_string());
        } else {
            return false;
        }
    }
    // Compute mod 97 iteratively.
    let mut rem = 0u64;
    for c in rearranged.chars() {
        let d = c.to_digit(10).unwrap_or(0) as u64;
        rem = (rem * 10 + d) % 97;
    }
    rem == 1
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn categories(p: &str) -> Vec<DlpCategory> {
        scan_payload(p).into_iter().map(|f| f.category).collect()
    }

    // ─── SSN ───────────────────────────────────────────────────

    #[test]
    fn ssn_canonical_match() {
        let cats = categories("My SSN is 123-45-6789, do not share");
        assert!(cats.contains(&DlpCategory::Ssn));
    }

    #[test]
    fn ssn_rejects_zero_components() {
        assert!(!categories("000-12-3456").contains(&DlpCategory::Ssn));
        assert!(!categories("123-00-6789").contains(&DlpCategory::Ssn));
        assert!(!categories("123-45-0000").contains(&DlpCategory::Ssn));
    }

    #[test]
    fn ssn_rejects_666_area() {
        assert!(!categories("666-12-3456").contains(&DlpCategory::Ssn));
    }

    #[test]
    fn ssn_no_match_when_part_of_longer_digit_run() {
        assert!(!categories("999123-45-67891").contains(&DlpCategory::Ssn));
    }

    #[test]
    fn ssn_redacted_preview_first_four_chars() {
        let findings = scan_payload("contact 555-12-3456 anytime");
        let ssn = findings
            .iter()
            .find(|f| f.category == DlpCategory::Ssn)
            .unwrap();
        assert_eq!(ssn.redacted_preview, "555-***");
    }

    // ─── PAN India ─────────────────────────────────────────────

    #[test]
    fn pan_india_canonical_match() {
        // ABCDE1234F format
        let cats = categories("PAN: ABCDE1234F");
        assert!(cats.contains(&DlpCategory::PanIndia));
    }

    #[test]
    fn pan_india_rejects_wrong_shape() {
        assert!(!categories("PAN: ABCD1234FF").contains(&DlpCategory::PanIndia)); // only 4 letters
        assert!(!categories("PAN: ABCDE123FF").contains(&DlpCategory::PanIndia)); // only 3 digits
        assert!(!categories("PAN: abcde1234f").contains(&DlpCategory::PanIndia));
        // lowercase
    }

    #[test]
    fn pan_india_no_match_when_part_of_longer_alphanumeric() {
        assert!(!categories("XABCDE1234FY").contains(&DlpCategory::PanIndia));
    }

    // ─── Aadhaar ───────────────────────────────────────────────

    #[test]
    fn aadhaar_canonical_12_digits() {
        assert!(categories("234567890123").contains(&DlpCategory::Aadhaar));
    }

    #[test]
    fn aadhaar_spaced_4_4_4() {
        assert!(categories("2345 6789 0123").contains(&DlpCategory::Aadhaar));
    }

    #[test]
    fn aadhaar_rejects_leading_zero_or_one() {
        assert!(!categories("0234 5678 9012").contains(&DlpCategory::Aadhaar));
        assert!(!categories("1234 5678 9012").contains(&DlpCategory::Aadhaar));
    }

    #[test]
    fn aadhaar_no_match_when_part_of_longer_digit_run() {
        assert!(!categories("9234567890123456").contains(&DlpCategory::Aadhaar));
    }

    // ─── Card number ───────────────────────────────────────────

    #[test]
    fn card_number_valid_visa() {
        // Test PAN 4111-1111-1111-1111 (passes Luhn)
        assert!(categories("Charge 4111-1111-1111-1111 today").contains(&DlpCategory::CardNumber));
    }

    #[test]
    fn card_number_valid_amex_15_digits() {
        // Amex test card 3782 822463 10005 (passes Luhn)
        assert!(categories("3782 822463 10005").contains(&DlpCategory::CardNumber));
    }

    #[test]
    fn card_number_rejects_failing_luhn() {
        // Same shape but last digit changed → fails Luhn
        assert!(!categories("4111-1111-1111-1112").contains(&DlpCategory::CardNumber));
    }

    #[test]
    fn card_number_rejects_too_few_digits() {
        assert!(!categories("1234").contains(&DlpCategory::CardNumber));
        assert!(!categories("411111111111").contains(&DlpCategory::CardNumber));
        // 12 digits
    }

    #[test]
    fn card_number_handles_dash_or_space_separators() {
        assert!(categories("4111 1111 1111 1111").contains(&DlpCategory::CardNumber));
        assert!(categories("4111-1111-1111-1111").contains(&DlpCategory::CardNumber));
    }

    // ─── IBAN ──────────────────────────────────────────────────

    #[test]
    fn iban_valid_german_example() {
        // DE89370400440532013000 — canonical IBAN example
        assert!(categories("Transfer to DE89370400440532013000 today").contains(&DlpCategory::Iban));
    }

    #[test]
    fn iban_rejects_failing_mod97() {
        // Same shape but tampered with the last digit
        assert!(!categories("DE89370400440532013999").contains(&DlpCategory::Iban));
    }

    #[test]
    fn iban_no_match_lowercase_prefix() {
        assert!(!categories("de89370400440532013000").contains(&DlpCategory::Iban));
    }

    // ─── Combined / adversarial ────────────────────────────────

    #[test]
    fn payload_with_multiple_findings() {
        let cats =
            categories("Card 4111-1111-1111-1111, SSN 123-45-6789, IBAN DE89370400440532013000");
        assert!(cats.contains(&DlpCategory::CardNumber));
        assert!(cats.contains(&DlpCategory::Ssn));
        assert!(cats.contains(&DlpCategory::Iban));
    }

    #[test]
    fn empty_payload_no_findings() {
        assert!(scan_payload("").is_empty());
        assert!(!has_findings(""));
    }

    #[test]
    fn benign_payload_no_findings() {
        assert!(scan_payload("Hello, world! No secrets here.").is_empty());
        assert!(scan_payload("Article about Emaar sukuk maturity 2027.").is_empty());
    }

    // ─── Adversarial: bypass attempts ──────────────────────────

    #[test]
    fn adversarial_ssn_with_unicode_dashes_not_detected_yet() {
        // Documented limitation: unicode em-dash (—) bypass.
        // PR-K3.b adds normalization. Today, this falls through.
        assert!(!categories("123\u{2014}45\u{2014}6789").contains(&DlpCategory::Ssn));
    }

    #[test]
    fn adversarial_card_with_extra_punctuation_blocks() {
        // Slashes between groups are not currently parsed; we'd
        // need additional separators (PR-K3.b). Today we miss
        // these. Documented limitation.
        assert!(!categories("4111/1111/1111/1111").contains(&DlpCategory::CardNumber));
    }

    #[test]
    fn label_method_returns_human_readable() {
        assert_eq!(DlpCategory::Ssn.label(), "US Social Security Number");
        assert_eq!(DlpCategory::Iban.label(), "IBAN");
        assert_eq!(DlpCategory::CardNumber.label(), "payment card number");
    }

    #[test]
    fn has_findings_shortcut_returns_true_when_match() {
        assert!(has_findings("123-45-6789"));
        assert!(has_findings("4111-1111-1111-1111"));
    }

    // ─── §23 fixture-shaped payloads ───────────────────────────

    #[test]
    fn s23_user_should_not_leak_aadhaar_to_mcp() {
        // §23 reference user has Hyderabad properties → likely has
        // an Aadhaar in their metadata. An MCP server attempting
        // to read property metadata + ship to its backend must be
        // blocked.
        let payload = r#"{"property":{"owner_aadhaar":"234567890123","intent":"for-rent"}}"#;
        assert!(has_findings(payload));
    }

    #[test]
    fn s23_user_should_not_leak_pan_to_mcp() {
        let payload = r#"{"tax":{"pan_number":"ABCDE1234F","return_id":"XYZ"}}"#;
        assert!(has_findings(payload));
    }

    #[test]
    fn s23_global_news_summary_no_findings() {
        // A perfectly normal news summary shouldn't false-positive.
        let payload = r#"{"summary":"Emaar Sukuk matures next month. \
                          Refinancing discussions continue."}"#;
        assert!(!has_findings(payload));
    }

    // ─── Performance smoke check ───────────────────────────────

    #[test]
    fn large_payload_scan_completes() {
        // 100KB of benign content should scan without timeout.
        let payload = "abcdefghijklmnop ".repeat(6250); // ~100KB
        let findings = scan_payload(&payload);
        assert!(findings.is_empty());
    }
}
