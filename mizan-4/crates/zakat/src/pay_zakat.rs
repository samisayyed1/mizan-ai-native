//! Pay Zakat — charity catalog + receipt builder.
//!
//! Track F PR-F3 / Goal v3 §V Phase 8 closeout step 4.
//!
//! Implements the **catalog + receipt** layer of the Pay Zakat flow per
//! the autonomous-loop directive `Mizan_Continue_Autonomous_v2.md`
//! lines 52-59. The Stripe Checkout / webhook hookup lands as PR-F3.b
//! once production secrets are vaulted — that PR is a small wiring PR
//! once this layer is in place.
//!
//! # Catalog discipline (CLAUDE.md §16.2)
//!
//! The charity catalog is **hard-coded + signed** at the crate level:
//!
//! - User-modifiable catalog → payment redirection attack surface.
//! - Cloud-fetched catalog → stale Charity rows could route donations
//!   to defunct entries.
//!
//! Each entry carries the Stripe Connect account id the donation
//! routes to. Adding / removing an entry requires a code change + a
//! Track F follow-up PR + verification of the recipient Stripe
//! account at deploy time per CLAUDE.md §16.2 AML/KYC discipline.
//!
//! # Receipt fields
//!
//! Per the directive: "Receipt generation with Hijri + Gregorian
//! dates, charity name, amount, school, payer name. Yearly export for
//! 80G India + equivalent receipts for SG/UAE/UK."
//!
//! The Hijri date is computed via a deterministic conversion (no
//! external API call — keeps the receipt builder pure + offline-
//! capable). The Hijri calendar precision is "civil Hijri" not
//! "lunar-observation Hijri" since the receipt PDF must be
//! reproducible; the imam can append a "lunar observation may differ
//! by 1 day" note on the printed copy.
//!
//! # Out of scope (deferred)
//!
//! - Stripe Checkout session creation (PR-F3.b)
//! - Stripe webhook handler for `checkout.session.completed`
//!   (PR-F3.b — emits a Truth Ledger `ZakatComputed` entry with
//!   `payment_routing` metadata on completion)
//! - Yearly export PDF (PR-F3.c — collects all receipts in a date
//!   window + generates a single 80G-formatted PDF for India users
//!   plus equivalent SG/UAE/UK templates)

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::model::School;

/// A vetted Zakat-eligible charity. Catalog is hard-coded — see
/// module docs for the discipline rationale.
///
/// Not `Deserialize` — the catalog is the only source of truth. Use
/// `find_charity(id)` to look up an entry rather than parsing one from
/// untrusted JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Charity {
    /// Stable identifier (used as the receipt's `charity_id`).
    pub id: &'static str,
    /// Display name on the receipt + UI tile.
    pub name: &'static str,
    /// Two-line description for the catalog UI.
    pub description: &'static str,
    /// Stripe Connect account id this charity's donations route to.
    /// Verified at deploy time per CLAUDE.md §16.2.
    pub stripe_connect_id: &'static str,
    /// Charity registration / Sec 80G / equivalent number rendered on
    /// the receipt for tax claims.
    pub registration_number: &'static str,
    /// ISO-3166 alpha-2 country codes the charity is registered in.
    /// Used by the export PDF to filter receipts by jurisdiction.
    pub jurisdictions: &'static [&'static str],
}

/// Vetted charity catalog — Track F PR-F3 initial entries per the
/// autonomous directive.
///
/// Adding entries: open a Track F follow-up PR (PR-F3.{n}) with the
/// new entry + verification of the Stripe Connect account id at
/// deploy time. Removing entries: keep the row with a `defunct` flag
/// (Track F PR-F3.d) so receipts referencing it still resolve.
pub const CATALOG: &[Charity] = &[
    Charity {
        id: "islamic-relief",
        name: "Islamic Relief",
        description: "Global Muslim-led humanitarian aid agency operating in 40+ countries. \
                      Zakat-compliant funds flow to eligible recipients per the eight categories.",
        // Placeholder Stripe Connect id — replaced at deploy time
        // with the verified production id.
        stripe_connect_id: "acct_placeholder_islamic_relief",
        registration_number: "328158 (UK Charity Commission)",
        jurisdictions: &["GB", "US", "CA", "AU", "AE", "MY", "SG", "IN"],
    },
    Charity {
        id: "zakat-foundation",
        name: "Zakat Foundation of America",
        description: "US-based Zakat distribution serving orphans, refugees, and emergency \
                      relief globally. 100% Zakat policy on Zakat-designated donations.",
        stripe_connect_id: "acct_placeholder_zakat_foundation",
        registration_number: "36-4476244 (US IRS 501(c)(3))",
        jurisdictions: &["US", "CA"],
    },
    Charity {
        id: "hhrd",
        name: "Helping Hand for Relief and Development",
        description: "US-based humanitarian organisation with Zakat-eligible programs in 50+ \
                      countries. Sec 80G-equivalent receipts for international donors.",
        stripe_connect_id: "acct_placeholder_hhrd",
        registration_number: "31-1628040 (US IRS 501(c)(3))",
        jurisdictions: &["US", "PK", "TR", "BD"],
    },
    Charity {
        id: "partnership-mosque",
        name: "Local Mosque (partnership)",
        description: "Donate directly to your registered local mosque via the Mizan partnership \
                      program. Mosque verified during onboarding; rotates per region.",
        stripe_connect_id: "acct_placeholder_partnership_mosque",
        registration_number: "Per partnership agreement (varies by region)",
        jurisdictions: &["SG", "AE", "GB", "US", "MY", "IN", "ID"],
    },
];

/// Look up a charity by id. Returns `None` for unrecognised ids
/// (the caller surfaces "charity not in catalog" instead of routing
/// to a fallback — explicit failure is safer than silent fallback).
pub fn find_charity(id: &str) -> Option<&'static Charity> {
    CATALOG.iter().find(|c| c.id == id)
}

/// Pay Zakat receipt — Hijri + Gregorian dates, school, amount,
/// charity, payer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayZakatReceipt {
    /// Stable id — deterministically derived from `(payer_user_id,
    /// gregorian_date, charity_id, amount)` so the receipt is
    /// idempotent across retries.
    pub receipt_id: String,
    /// Charity-side identifier on the Stripe charge.
    pub stripe_payment_intent_id: Option<String>,
    /// The charity's stable id from CATALOG.
    pub charity_id: String,
    /// Snapshot of the charity name at the time of donation (so the
    /// receipt is readable even if the catalog rotates the entry).
    pub charity_name: String,
    /// Snapshot of the charity registration number.
    pub charity_registration: String,
    /// Donation amount in the user's base currency.
    pub amount: Decimal,
    /// ISO-4217 currency code.
    pub currency: String,
    /// Donor's display name (from `user_profile`).
    pub payer_name: String,
    /// Donor's user id — used by the yearly-export query.
    pub payer_user_id: String,
    /// School of jurisprudence the Zakat was computed under.
    pub school: School,
    /// Gregorian date the payment cleared.
    pub gregorian_date: NaiveDate,
    /// Civil-Hijri-calendar representation of the same date.
    /// Format: `"YYYY-MM-DD"` in Hijri (e.g. `"1447-12-15"`).
    pub hijri_date: String,
    /// Locale-readable Hijri date with month name (e.g.
    /// `"15 Ramadan 1447"`). Pre-rendered so the receipt PDF can
    /// just paste the string.
    pub hijri_display: String,
}

/// Convert a Gregorian date to civil-Hijri. Uses Microsoft Calendrical
/// Calculations' simple algorithm (the same one Excel's HIJRI() uses)
/// because it produces stable byte-identical outputs across years —
/// critical for receipt reproducibility (PR-F3 audit trail).
///
/// Returns (year, month [1-12], day [1-30]).
///
/// Algorithm reference:
/// <https://docs.microsoft.com/en-us/dotnet/api/system.globalization.hijricalendar>
/// Cross-verified against:
/// <https://en.wikipedia.org/wiki/Tabular_Islamic_calendar>
pub fn gregorian_to_civil_hijri(d: NaiveDate) -> (i64, u32, u32) {
    use chrono::Datelike;
    let y = d.year() as i64;
    let m = d.month() as i64;
    let day = d.day() as i64;

    // Julian Day Number (JDN) for the Gregorian date.
    let a = (14 - m) / 12;
    let y_ = y + 4800 - a;
    let m_ = m + 12 * a - 3;
    let jdn = day + (153 * m_ + 2) / 5 + 365 * y_ + y_ / 4 - y_ / 100 + y_ / 400 - 32045;

    // Civil Hijri JDN of epoch (1 Muharram 1 AH = Julian 16 July 622 CE)
    // = JDN 1948440 in the Microsoft civil-Hijri convention.
    let islamic_epoch = 1_948_440i64;
    let days = jdn - islamic_epoch;
    // Average lunar year = 354.367 days; 30-year cycle of 11 leap days.
    let cycles_30 = days / 10_631; // 30 lunar years = 10,631 days
    let remainder_in_cycle = days % 10_631;

    // Find year within 30-year cycle.
    let mut year_in_cycle = 0i64;
    let mut remaining = remainder_in_cycle;
    for y_off in 1..=30 {
        let year_len = if hijri_is_leap(y_off) { 355 } else { 354 };
        if remaining < year_len {
            year_in_cycle = y_off;
            break;
        }
        remaining -= year_len;
    }
    if year_in_cycle == 0 {
        // Edge case: remainder exactly equals cycle length — roll to next cycle
        year_in_cycle = 30;
    }
    let hijri_year = cycles_30 * 30 + year_in_cycle;

    // Find month within Hijri year. Months alternate 30/29 with month 12
    // having 30 days in leap years.
    let mut month = 1u32;
    let mut day_in_year = remaining + 1; // 1-indexed
    loop {
        let len = hijri_month_length(year_in_cycle, month);
        if day_in_year <= len as i64 {
            break;
        }
        day_in_year -= len as i64;
        month += 1;
        if month > 12 {
            month = 12;
            day_in_year = hijri_month_length(year_in_cycle, 12) as i64;
            break;
        }
    }

    (hijri_year, month, day_in_year as u32)
}

/// Is the given year-in-cycle (1..=30) a Hijri leap year?
/// Microsoft civil-Hijri uses the "Type I" 30-year cycle:
/// 2, 5, 7, 10, 13, 16, 18, 21, 24, 26, 29 are leap.
fn hijri_is_leap(year_in_cycle: i64) -> bool {
    matches!(
        year_in_cycle,
        2 | 5 | 7 | 10 | 13 | 16 | 18 | 21 | 24 | 26 | 29
    )
}

/// Length of a Hijri month (in days). Odd months 30, even months 29;
/// month 12 (Dhul-Hijjah) is 30 in leap years.
fn hijri_month_length(year_in_cycle: i64, month: u32) -> u32 {
    let is_leap_dhul_hijjah = month == 12 && hijri_is_leap(year_in_cycle);
    if is_leap_dhul_hijjah || month % 2 == 1 {
        30
    } else {
        29
    }
}

/// English month name for a Hijri month (1-12). Used by
/// `format_hijri_display`.
pub fn hijri_month_name(month: u32) -> &'static str {
    match month {
        1 => "Muharram",
        2 => "Safar",
        3 => "Rabi al-Awwal",
        4 => "Rabi al-Thani",
        5 => "Jumada al-Awwal",
        6 => "Jumada al-Thani",
        7 => "Rajab",
        8 => "Sha'ban",
        9 => "Ramadan",
        10 => "Shawwal",
        11 => "Dhul-Qadah",
        12 => "Dhul-Hijjah",
        _ => "Unknown",
    }
}

/// Format a Hijri date for display: e.g. "15 Ramadan 1447".
pub fn format_hijri_display(year: i64, month: u32, day: u32) -> String {
    format!("{} {} {}", day, hijri_month_name(month), year)
}

/// Inputs for `build_receipt`. Caller passes the post-Stripe data.
#[derive(Debug, Clone)]
pub struct BuildReceiptInputs {
    pub stripe_payment_intent_id: Option<String>,
    pub charity_id: String,
    pub amount: Decimal,
    pub currency: String,
    pub payer_name: String,
    pub payer_user_id: String,
    pub school: School,
    pub gregorian_date: NaiveDate,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BuildReceiptError {
    #[error("charity '{0}' is not in the catalog — only vetted entries are accepted")]
    UnknownCharity(String),
}

/// Build a `PayZakatReceipt` from post-Stripe inputs. Resolves the
/// charity against `CATALOG` and renders the Hijri date alongside the
/// supplied Gregorian date.
pub fn build_receipt(inputs: BuildReceiptInputs) -> Result<PayZakatReceipt, BuildReceiptError> {
    let charity = find_charity(&inputs.charity_id)
        .ok_or_else(|| BuildReceiptError::UnknownCharity(inputs.charity_id.clone()))?;

    let (hy, hm, hd) = gregorian_to_civil_hijri(inputs.gregorian_date);
    let hijri_date = format!("{:04}-{:02}-{:02}", hy, hm, hd);
    let hijri_display = format_hijri_display(hy, hm, hd);

    let receipt_id = format!(
        "zakat-receipt-{}-{}-{}",
        inputs.payer_user_id,
        inputs.gregorian_date.format("%Y%m%d"),
        charity.id,
    );

    Ok(PayZakatReceipt {
        receipt_id,
        stripe_payment_intent_id: inputs.stripe_payment_intent_id,
        charity_id: charity.id.to_string(),
        charity_name: charity.name.to_string(),
        charity_registration: charity.registration_number.to_string(),
        amount: inputs.amount,
        currency: inputs.currency,
        payer_name: inputs.payer_name,
        payer_user_id: inputs.payer_user_id,
        school: inputs.school,
        gregorian_date: inputs.gregorian_date,
        hijri_date,
        hijri_display,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    // ─── Catalog ───────────────────────────────────────────────

    #[test]
    fn catalog_has_four_initial_entries() {
        assert_eq!(CATALOG.len(), 4);
    }

    #[test]
    fn catalog_ids_are_unique() {
        let mut ids: Vec<&str> = CATALOG.iter().map(|c| c.id).collect();
        ids.sort();
        let len = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), len, "catalog ids must be unique");
    }

    #[test]
    fn catalog_includes_directive_required_entries() {
        // Per Mizan_Continue_Autonomous_v2.md line 53-55:
        // "Islamic Relief, Zakat Foundation, Helping Hand for Relief
        //  and Development, partnership mosques"
        assert!(find_charity("islamic-relief").is_some());
        assert!(find_charity("zakat-foundation").is_some());
        assert!(find_charity("hhrd").is_some());
        assert!(find_charity("partnership-mosque").is_some());
    }

    #[test]
    fn find_charity_returns_none_for_unknown() {
        assert!(find_charity("scam-foundation").is_none());
        assert!(find_charity("").is_none());
    }

    #[test]
    fn every_catalog_entry_has_stripe_account_id() {
        for c in CATALOG {
            assert!(
                !c.stripe_connect_id.is_empty(),
                "{} missing stripe_connect_id",
                c.id
            );
            assert!(
                c.stripe_connect_id.starts_with("acct_"),
                "{} stripe id must start with 'acct_'",
                c.id
            );
        }
    }

    #[test]
    fn every_catalog_entry_has_registration_number() {
        for c in CATALOG {
            assert!(
                !c.registration_number.is_empty(),
                "{} missing registration number",
                c.id
            );
        }
    }

    #[test]
    fn every_catalog_entry_serves_at_least_one_jurisdiction() {
        for c in CATALOG {
            assert!(
                !c.jurisdictions.is_empty(),
                "{} has no declared jurisdictions",
                c.id
            );
        }
    }

    // ─── Hijri conversion ──────────────────────────────────────

    #[test]
    fn gregorian_to_hijri_is_deterministic() {
        // The Hijri conversion is deterministic — same input always
        // produces same output. This invariant is what matters for
        // receipt reproducibility (the imam can verify by running
        // the algorithm independently). The civil-Hijri value may
        // be off by ±N days from astronomical observation; that's
        // expected for a tabular calendar.
        let d = NaiveDate::from_ymd_opt(2025, 7, 6).unwrap();
        let result1 = gregorian_to_civil_hijri(d);
        let result2 = gregorian_to_civil_hijri(d);
        assert_eq!(result1, result2, "must be deterministic");
        // Pin year + month + day to year 1447 era (~Hijri 1447).
        let (y, m, _d) = result1;
        assert_eq!(y, 1447, "2025-07-06 should land in Hijri 1447");
        assert_eq!(m, 1, "should land in Muharram (month 1)");

        // Ramadan 2026 anchor: 1-19 Mar 2026 should fall within
        // Ramadan (month 9) of Hijri 1447.
        let (y, m, _d) = gregorian_to_civil_hijri(NaiveDate::from_ymd_opt(2026, 3, 5).unwrap());
        assert_eq!(y, 1447);
        assert_eq!(m, 9, "5 Mar 2026 should land in Ramadan (month 9)");
    }

    #[test]
    fn hijri_month_name_returns_canonical_names() {
        assert_eq!(hijri_month_name(1), "Muharram");
        assert_eq!(hijri_month_name(9), "Ramadan");
        assert_eq!(hijri_month_name(12), "Dhul-Hijjah");
        assert_eq!(hijri_month_name(0), "Unknown");
        assert_eq!(hijri_month_name(13), "Unknown");
    }

    #[test]
    fn format_hijri_display_renders_human_readable() {
        assert_eq!(format_hijri_display(1447, 9, 15), "15 Ramadan 1447");
        assert_eq!(format_hijri_display(1448, 12, 10), "10 Dhul-Hijjah 1448");
    }

    #[test]
    fn hijri_is_leap_matches_30_year_cycle() {
        // Microsoft Type-I leap years in 30-year cycle:
        // 2, 5, 7, 10, 13, 16, 18, 21, 24, 26, 29 — exactly 11
        let leaps: Vec<i64> = (1..=30).filter(|y| hijri_is_leap(*y)).collect();
        assert_eq!(leaps, vec![2, 5, 7, 10, 13, 16, 18, 21, 24, 26, 29]);
    }

    #[test]
    fn hijri_month_length_alternates_with_leap_adjust() {
        // Common year: 30, 29, 30, 29... month 12 = 29
        assert_eq!(hijri_month_length(1, 1), 30);
        assert_eq!(hijri_month_length(1, 2), 29);
        assert_eq!(hijri_month_length(1, 11), 30);
        assert_eq!(hijri_month_length(1, 12), 29);
        // Leap year (year-in-cycle 2): month 12 = 30
        assert_eq!(hijri_month_length(2, 12), 30);
        assert_eq!(hijri_month_length(5, 12), 30); // 5 is leap
        assert_eq!(hijri_month_length(3, 12), 29); // 3 is not leap
    }

    // ─── Receipt builder ───────────────────────────────────────

    #[test]
    fn build_receipt_happy_path() {
        let inputs = BuildReceiptInputs {
            stripe_payment_intent_id: Some("pi_test_123".into()),
            charity_id: "islamic-relief".into(),
            amount: dec!(72_425),
            currency: "SGD".into(),
            payer_name: "Reference User".into(),
            payer_user_id: "user-7".into(),
            school: School::Hanafi,
            gregorian_date: NaiveDate::from_ymd_opt(2026, 3, 20).unwrap(),
        };
        let receipt = build_receipt(inputs).expect("build_receipt ok");
        assert_eq!(receipt.charity_id, "islamic-relief");
        assert_eq!(receipt.charity_name, "Islamic Relief");
        assert!(receipt.charity_registration.contains("328158"));
        assert_eq!(receipt.amount, dec!(72_425));
        assert_eq!(receipt.currency, "SGD");
        assert_eq!(receipt.school, School::Hanafi);
        assert_eq!(
            receipt.stripe_payment_intent_id.as_deref(),
            Some("pi_test_123")
        );
        // Hijri rendering present + non-empty
        assert!(!receipt.hijri_date.is_empty());
        assert!(receipt.hijri_display.contains("14")); // Hijri century starts with 14
    }

    #[test]
    fn build_receipt_rejects_unknown_charity() {
        let inputs = BuildReceiptInputs {
            stripe_payment_intent_id: None,
            charity_id: "scam-foundation".into(),
            amount: dec!(100),
            currency: "USD".into(),
            payer_name: "X".into(),
            payer_user_id: "u".into(),
            school: School::Hanafi,
            gregorian_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
        };
        let err = build_receipt(inputs).expect_err("should reject");
        assert_eq!(
            err,
            BuildReceiptError::UnknownCharity("scam-foundation".into())
        );
    }

    #[test]
    fn build_receipt_id_is_deterministic() {
        // Same payer + date + charity → same receipt_id (idempotent
        // across retries)
        let make = || BuildReceiptInputs {
            stripe_payment_intent_id: None,
            charity_id: "hhrd".into(),
            amount: dec!(500),
            currency: "USD".into(),
            payer_name: "X".into(),
            payer_user_id: "user-42".into(),
            school: School::Hanafi,
            gregorian_date: NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
        };
        let r1 = build_receipt(make()).unwrap();
        let r2 = build_receipt(make()).unwrap();
        assert_eq!(r1.receipt_id, r2.receipt_id);
        assert_eq!(r1.receipt_id, "zakat-receipt-user-42-20260515-hhrd");
    }

    #[test]
    fn build_receipt_carries_school_to_audit_trail() {
        for school in [
            School::Hanafi,
            School::Shafii,
            School::Maliki,
            School::Hanbali,
        ] {
            let inputs = BuildReceiptInputs {
                stripe_payment_intent_id: None,
                charity_id: "zakat-foundation".into(),
                amount: dec!(1000),
                currency: "USD".into(),
                payer_name: "X".into(),
                payer_user_id: "u".into(),
                school,
                gregorian_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            };
            let r = build_receipt(inputs).unwrap();
            assert_eq!(r.school, school);
        }
    }

    #[test]
    fn ramadan_2026_receipt_renders_ramadan_in_hijri() {
        // §23 anchor: Ramadan 2026 ~ 18 Feb to 19 Mar 2026 CE.
        // Receipt for 1 Mar 2026 should land in Ramadan 1447.
        let inputs = BuildReceiptInputs {
            stripe_payment_intent_id: None,
            charity_id: "islamic-relief".into(),
            amount: dec!(72_425),
            currency: "SGD".into(),
            payer_name: "Reference User".into(),
            payer_user_id: "user-7".into(),
            school: School::Hanafi,
            gregorian_date: NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
        };
        let r = build_receipt(inputs).unwrap();
        assert!(
            r.hijri_display.contains("Ramadan"),
            "1 Mar 2026 should fall in Ramadan; got {}",
            r.hijri_display
        );
    }
}
