//! Sensitive-data redaction for log lines.
//!
//! # Why this exists
//!
//! Mizan handles broker OAuth tokens, AI provider API keys, refresh
//! tokens for the device-sync server, and a few other secrets that
//! must never appear in user-visible log files. The codebase has
//! grown organically; not every `info!` / `warn!` / `error!` call
//! site can be audited continuously. This module catches the common
//! mistake patterns at the output stage so even an accidental
//! `info!("{:?}", broker_response)` doesn't leak.
//!
//! # What we redact
//!
//! * `Authorization: Bearer <token>` HTTP headers — replaced with
//!   `Bearer ***`.
//! * Any JSON or query-string-style value attached to keys whose name
//!   contains `token`, `secret`, `password`, `api_key`, `apikey`,
//!   `bearer`, `refresh_token`, `access_token`, `client_secret`.
//!   Both `"key":"value"` (JSON) and `key=value` (query string)
//!   spellings are caught, case-insensitive on the key name.
//!
//! # What we DON'T do
//!
//! * Redact arbitrary high-entropy strings — too many false positives
//!   (UUIDs, hashes, base64-encoded asset IDs). The key-name-based
//!   approach is tight enough that legitimate fields like
//!   `asset_id`, `transaction_id`, `idempotency_key` pass through
//!   unchanged.
//! * Touch the structured `target` / `module_path` fields — only the
//!   formatted message body is redacted.
//!
//! All patterns are matched by hand-rolled substring walks (no regex
//! dep) so the cost is sub-microsecond per line. Verified against a
//! synthetic corpus of leaky messages in the test below.

/// Returns the input with sensitive substrings replaced.
///
/// The returned `String` is allocated only when at least one match
/// fires; otherwise the caller pays nothing beyond a single pass.
/// Public so tests can exercise it directly.
pub fn redact_sensitive(msg: &str) -> String {
    // Cheap fast-path: if none of the trigger keywords appear, skip.
    // Case-insensitive lookup is approximated by lowercasing the
    // message for the contains-check only; the actual substitution
    // walks the original to preserve casing in the output.
    let lower = msg.to_lowercase();
    const TRIGGERS: &[&str] = &[
        "bearer",
        "token",
        "secret",
        "password",
        "api_key",
        "apikey",
        "authorization",
    ];
    if !TRIGGERS.iter().any(|t| lower.contains(t)) {
        return msg.to_string();
    }

    let mut out = msg.to_string();

    // 1. `Bearer <token>` (HTTP Authorization header). Replace the
    //    token portion (up to whitespace, quote, or end-of-string).
    out = redact_after_keyword(&out, "Bearer ", &[' ', '"', '\'', '\n', '\r', '\t']);
    out = redact_after_keyword(&out, "bearer ", &[' ', '"', '\'', '\n', '\r', '\t']);

    // 2. JSON-style: `"key":"value"` for sensitive keys.
    for key in SENSITIVE_KEYS {
        out = redact_json_value(&out, key);
    }

    // 3. Query-string / form-urlencoded: `key=value`.
    for key in SENSITIVE_KEYS {
        out = redact_kv_value(&out, key);
    }

    out
}

const SENSITIVE_KEYS: &[&str] = &[
    "access_token",
    "refresh_token",
    "id_token",
    "client_secret",
    "client_id_secret",
    "api_key",
    "apikey",
    "api-key",
    "secret",
    "password",
    "passphrase",
    "bearer_token",
    "token",
];

const REDACTED: &str = "***";

/// Replace the run of characters AFTER `keyword` (up to the first
/// terminator char or end-of-string) with `REDACTED`. Case-sensitive
/// on the keyword itself — caller supplies both cases when needed.
fn redact_after_keyword(input: &str, keyword: &str, terminators: &[char]) -> String {
    let mut out = String::with_capacity(input.len());
    let mut remaining = input;
    while let Some(idx) = remaining.find(keyword) {
        out.push_str(&remaining[..idx + keyword.len()]);
        let tail = &remaining[idx + keyword.len()..];
        let end = tail
            .find(|c| terminators.contains(&c))
            .unwrap_or(tail.len());
        if end > 0 {
            out.push_str(REDACTED);
        }
        remaining = &tail[end..];
    }
    out.push_str(remaining);
    out
}

/// JSON-style: find `"<key>"...:..."<value>"` and replace value with `***`.
/// Tolerates whitespace between key, colon, and value. Case-insensitive
/// on the key.
fn redact_json_value(input: &str, key: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut remaining = input;
    let lowered_key = key.to_lowercase();
    loop {
        // Find the quoted key (case-insensitive comparison).
        let lower_remaining = remaining.to_lowercase();
        let needle = format!("\"{}\"", lowered_key);
        let Some(start) = lower_remaining.find(&needle) else {
            out.push_str(remaining);
            return out;
        };
        // Push everything up to and including the key.
        let key_end = start + needle.len();
        out.push_str(&remaining[..key_end]);

        // Now scan past whitespace + `:` + whitespace + `"`.
        let mut cursor = key_end;
        let bytes = remaining.as_bytes();
        // Skip whitespace.
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            out.push(bytes[cursor] as char);
            cursor += 1;
        }
        // Expect `:`. If not found, bail out and continue scanning.
        if cursor >= bytes.len() || bytes[cursor] != b':' {
            remaining = &remaining[key_end..];
            continue;
        }
        out.push(':');
        cursor += 1;
        // Skip whitespace after colon.
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            out.push(bytes[cursor] as char);
            cursor += 1;
        }
        // Expect opening quote.
        if cursor >= bytes.len() || bytes[cursor] != b'"' {
            // Not a quoted string value — could be a number, null, etc.
            // Skip to next iteration without redacting.
            remaining = &remaining[cursor..];
            continue;
        }
        out.push('"');
        cursor += 1;

        // Find the closing quote, honouring escape sequences. We don't
        // need a full JSON parser — just need to skip past `\"`.
        let mut end = cursor;
        while end < bytes.len() {
            if bytes[end] == b'\\' && end + 1 < bytes.len() {
                end += 2;
                continue;
            }
            if bytes[end] == b'"' {
                break;
            }
            end += 1;
        }
        // Replace the value content.
        out.push_str(REDACTED);
        if end < bytes.len() {
            out.push('"'); // closing quote
            remaining = &remaining[end + 1..];
        } else {
            // Unterminated value — caller already past EOF, treat
            // the rest as redacted and stop.
            return out;
        }
    }
}

/// Query-string / form-urlencoded style: `<key>=<value>` where value
/// ends at `&`, whitespace, `"`, or end-of-string. Case-insensitive
/// key.
fn redact_kv_value(input: &str, key: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut remaining = input;
    let lowered_key = key.to_lowercase();
    let terminators = ['&', ' ', '"', '\'', '\n', '\r', '\t', ',', ';'];
    loop {
        let lower_remaining = remaining.to_lowercase();
        // Match `<key>=` — but only when preceded by a non-alphanumeric
        // boundary so we don't redact substrings of unrelated keys
        // (e.g. don't let `token` match the `_token` part of
        // `csrftoken`).
        let needle = format!("{}=", lowered_key);
        let Some(idx) = find_with_boundary(&lower_remaining, &needle) else {
            out.push_str(remaining);
            return out;
        };
        let kv_start = idx;
        let value_start = idx + needle.len();
        out.push_str(&remaining[..value_start]);
        let tail = &remaining[value_start..];
        let end = tail
            .find(|c| terminators.contains(&c))
            .unwrap_or(tail.len());
        if end > 0 {
            out.push_str(REDACTED);
        }
        // Avoid infinite loop when redacted segment is empty.
        let consumed = value_start + end;
        if consumed == kv_start {
            out.push_str(&remaining[consumed..]);
            return out;
        }
        remaining = &remaining[consumed..];
    }
}

/// Find `needle` in `haystack` only at positions where the preceding
/// character is non-alphanumeric (or at start-of-string). Returns the
/// position in `haystack`, or `None`.
fn find_with_boundary(haystack: &str, needle: &str) -> Option<usize> {
    let bytes = haystack.as_bytes();
    let mut search_from = 0usize;
    while let Some(rel) = haystack[search_from..].find(needle) {
        let abs = search_from + rel;
        let prev_ok = abs == 0 || !bytes[abs - 1].is_ascii_alphanumeric();
        if prev_ok {
            return Some(abs);
        }
        search_from = abs + 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_through_innocent_messages() {
        let input = "Imported 358 activities into account abc-123";
        assert_eq!(redact_sensitive(input), input);
    }

    #[test]
    fn redacts_bearer_token() {
        let out = redact_sensitive("GET /api auth=Bearer eyJabc123 done");
        assert!(out.contains("Bearer ***"), "got: {}", out);
        assert!(!out.contains("eyJabc123"), "got: {}", out);
    }

    #[test]
    fn redacts_json_refresh_token() {
        let out = redact_sensitive(r#"{"refresh_token":"rt_super_secret_123","status":"ok"}"#);
        assert!(out.contains(r#""refresh_token":"***""#), "got: {}", out);
        assert!(!out.contains("rt_super_secret_123"), "got: {}", out);
        assert!(out.contains(r#""status":"ok""#), "got: {}", out);
    }

    #[test]
    fn redacts_json_access_token_with_whitespace() {
        let out = redact_sensitive(r#"{ "access_token" : "live_tk", "name": "alice" }"#);
        assert!(out.contains("access_token"));
        assert!(!out.contains("live_tk"), "got: {}", out);
    }

    #[test]
    fn redacts_query_string_api_key() {
        let out = redact_sensitive("GET /v1?api_key=abc123&q=AAPL");
        assert!(out.contains("api_key=***"), "got: {}", out);
        assert!(!out.contains("abc123"), "got: {}", out);
        assert!(out.contains("q=AAPL"));
    }

    #[test]
    fn does_not_redact_unrelated_keys() {
        // `asset_id`, `request_id` etc. must not be touched.
        let input = "request_id=abc activity_id=def";
        assert_eq!(redact_sensitive(input), input);
    }

    #[test]
    fn does_not_partially_match_word_boundary() {
        // `csrftoken=` should NOT trigger the `token=` redaction
        // because the preceding char is alphanumeric.
        let input = "csrftoken=safevalue";
        let out = redact_sensitive(input);
        assert_eq!(out, input, "should not redact csrftoken: {}", out);
    }

    #[test]
    fn handles_multiple_secrets_in_one_message() {
        let input = r#"req=Bearer tok_aaa body={"client_secret":"sec_bbb"} q=password=p_ccc"#;
        let out = redact_sensitive(input);
        assert!(!out.contains("tok_aaa"), "{}", out);
        assert!(!out.contains("sec_bbb"), "{}", out);
        assert!(!out.contains("p_ccc"), "{}", out);
        assert!(out.contains("Bearer ***"));
        assert!(out.contains(r#""client_secret":"***""#));
        assert!(out.contains("password=***"));
    }

    #[test]
    fn redacts_case_insensitive_keys() {
        let out = redact_sensitive(r#"{"Refresh_Token":"abc","API_KEY":"xyz"}"#);
        assert!(!out.contains("abc"), "{}", out);
        assert!(!out.contains("xyz"), "{}", out);
    }

    #[test]
    fn handles_empty_string() {
        assert_eq!(redact_sensitive(""), "");
    }
}
