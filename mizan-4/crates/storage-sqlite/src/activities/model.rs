//! Database models for activities.

use chrono::{NaiveDate, NaiveDateTime, TimeZone, Utc};
use diesel::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use mizan_core::activities::{
    Activity, ActivityStatus, ActivityUpdate, ActivityUpsert, NewActivity,
};

/// Helper function to parse a string into a Decimal,
/// with a fallback for scientific notation by parsing as f64 first.
fn parse_decimal_string_tolerant(value_str: &str, field_name: &str) -> Decimal {
    match Decimal::from_str(value_str) {
        Ok(d) => d,
        Err(e_decimal) => match Decimal::from_scientific(value_str) {
            Ok(d) => d,
            Err(e_scientific) => {
                log::error!(
                    "Failed to parse {} '{}': as Decimal (err: {}), and as scientific (err: {}). Falling back to ZERO.",
                    field_name,
                    value_str,
                    e_decimal,
                    e_scientific
                );
                Decimal::ZERO
            }
        },
    }
}

/// Tolerant ISO-timestamp parser. Accepts:
///   - RFC3339 "2026-05-25T15:16:19Z" / "2026-05-25T15:16:19+00:00"
///   - SQLite default "2026-05-25 15:16:19" (space separator, no tz)
///   - Bare ISO date "2026-05-25" (midnight UTC)
///
/// Falls back to the Unix epoch + loud error log on parse failure
/// rather than `Utc::now()` so a broken row doesn't silently masquerade
/// as fresh data (the same failure mode QA Pass 3 found on activity_date).
fn parse_timestamp_tolerant(s: &str, field: &str) -> chrono::DateTime<Utc> {
    use chrono::{DateTime, NaiveDate, NaiveDateTime};

    // 1) RFC3339 (the writer's canonical format)
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return dt.with_timezone(&Utc);
    }
    // 2) SQLite default "YYYY-MM-DD HH:MM:SS"
    if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Utc.from_utc_datetime(&naive);
    }
    // 3) SQLite "YYYY-MM-DD HH:MM:SS.f..."
    if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f") {
        return Utc.from_utc_datetime(&naive);
    }
    // 4) Bare ISO date
    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        if let Some(t) = date.and_hms_opt(0, 0, 0) {
            return Utc.from_utc_datetime(&t);
        }
    }
    log::error!(
        "Failed to parse {} '{}'. Falling back to epoch — an obvious wrong value the operator \
         will notice, instead of silently flattening to Utc::now().",
        field,
        s
    );
    DateTime::<Utc>::from_timestamp(0, 0).expect("epoch is a valid timestamp")
}

/// Database model for activities - COMPLETELY REDESIGNED
#[derive(
    Queryable,
    Identifiable,
    Insertable,
    AsChangeset,
    Selectable,
    PartialEq,
    Serialize,
    Deserialize,
    Debug,
    Clone,
    Default,
)]
#[diesel(table_name = crate::schema::activities)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ActivityDB {
    pub id: String,
    pub account_id: String,
    pub asset_id: Option<String>, // NOW NULLABLE

    // Classification
    pub activity_type: String,
    pub activity_type_override: Option<String>,
    pub source_type: Option<String>,
    pub subtype: Option<String>,
    pub status: String,

    // Timing
    pub activity_date: String,
    pub settlement_date: Option<String>,

    // Quantities - NOW ALL NULLABLE
    // treat_none_as_null: When None, Diesel sets column to NULL instead of skipping
    #[diesel(treat_none_as_null = true)]
    pub quantity: Option<String>,
    #[diesel(treat_none_as_null = true)]
    pub unit_price: Option<String>,
    #[diesel(treat_none_as_null = true)]
    pub amount: Option<String>,
    #[diesel(treat_none_as_null = true)]
    pub fee: Option<String>,
    pub currency: String,
    #[diesel(treat_none_as_null = true)]
    pub fx_rate: Option<String>,

    // Metadata
    #[diesel(treat_none_as_null = true)]
    pub notes: Option<String>,
    pub metadata: Option<String>,

    // Source identity
    pub source_system: Option<String>,
    pub source_record_id: Option<String>,
    pub source_group_id: Option<String>,
    pub idempotency_key: Option<String>,
    pub import_run_id: Option<String>,

    // Sync flags (i32 for SQLite INTEGER)
    pub is_user_modified: i32,
    pub needs_review: i32,

    // Audit
    pub created_at: String,
    pub updated_at: String,
}

/// Model for activity details including related data
/// Field order MUST match the select() order in repository.rs
#[derive(Queryable, QueryableByName, Serialize, Deserialize, Clone, Debug)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[serde(rename_all = "camelCase")]
pub struct ActivityDetailsDB {
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub id: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub account_id: String,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    pub asset_id: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub activity_type: String,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    pub subtype: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub status: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub date: String,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    pub quantity: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    pub unit_price: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub currency: String,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    pub fee: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    pub amount: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    pub notes: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    pub fx_rate: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    pub needs_review: i32,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    pub is_user_modified: i32,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    pub source_system: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    pub source_record_id: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    pub source_group_id: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    pub idempotency_key: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    pub import_run_id: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub created_at: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub updated_at: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub account_name: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub account_currency: String,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    pub asset_symbol: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    pub asset_name: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    pub exchange_mic: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    pub asset_pricing_mode: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    pub instrument_type: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    pub metadata: Option<String>,
}

impl ActivityDetailsDB {
    pub fn get_quantity(&self) -> Decimal {
        self.quantity
            .as_ref()
            .map(|s| parse_decimal_string_tolerant(s, "quantity"))
            .unwrap_or(Decimal::ZERO)
    }

    pub fn get_unit_price(&self) -> Decimal {
        self.unit_price
            .as_ref()
            .map(|s| parse_decimal_string_tolerant(s, "unit_price"))
            .unwrap_or(Decimal::ZERO)
    }

    pub fn get_fee(&self) -> Decimal {
        self.fee
            .as_ref()
            .map(|s| parse_decimal_string_tolerant(s, "fee"))
            .unwrap_or(Decimal::ZERO)
    }

    pub fn get_amount(&self) -> Option<Decimal> {
        self.amount
            .as_ref()
            .map(|s| parse_decimal_string_tolerant(s, "amount"))
    }
}

/// Database model for account → import template association
#[derive(
    Debug, Clone, Serialize, Deserialize, Queryable, Identifiable, AsChangeset, Insertable,
)]
#[diesel(table_name = crate::schema::import_account_templates)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[serde(rename_all = "camelCase")]
pub struct ImportAccountTemplateDB {
    pub id: String,
    pub account_id: String,
    pub context_kind: String,
    pub source_system: String,
    pub template_id: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(
    Debug, Clone, Serialize, Deserialize, Queryable, Identifiable, AsChangeset, Insertable,
)]
#[diesel(primary_key(id))]
#[diesel(table_name = crate::schema::import_templates)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[serde(rename_all = "camelCase")]
pub struct ImportTemplateDB {
    pub id: String,
    pub name: String,
    pub scope: String,
    pub kind: String,
    pub source_system: String,
    pub config_version: i32,
    pub config: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

/// Database model for income data query results
#[derive(Debug, Serialize, QueryableByName)]
#[serde(rename_all = "camelCase")]
#[diesel(table_name = crate::schema::activities)]
pub struct IncomeDataDB {
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub date: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub income_type: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub symbol: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub symbol_name: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub currency: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub amount: String,
}

impl IncomeDataDB {
    pub fn get_amount(&self) -> Decimal {
        parse_decimal_string_tolerant(&self.amount, "amount")
    }
}

impl From<ActivityDetailsDB> for mizan_core::activities::ActivityDetails {
    fn from(db: ActivityDetailsDB) -> Self {
        use mizan_core::activities::ActivityStatus;

        // Parse status string to ActivityStatus enum
        let status = match db.status.as_str() {
            "POSTED" => ActivityStatus::Posted,
            "PENDING" => ActivityStatus::Pending,
            "DRAFT" => ActivityStatus::Draft,
            "VOID" => ActivityStatus::Void,
            _ => ActivityStatus::Posted, // Default to Posted for unknown values
        };

        let amount = db.amount.or_else(|| {
            let q = db.quantity.as_ref()?;
            let p = db.unit_price.as_ref()?;
            let qty = parse_decimal_string_tolerant(q, "quantity");
            let price = parse_decimal_string_tolerant(p, "unit_price");
            Some((qty * price).to_string())
        });

        Self {
            id: db.id,
            account_id: db.account_id,
            asset_id: db.asset_id.unwrap_or_default(),
            activity_type: db.activity_type,
            subtype: db.subtype,
            status,
            date: db.date,
            quantity: db.quantity,
            unit_price: db.unit_price,
            currency: db.currency,
            fee: db.fee,
            amount,
            needs_review: db.needs_review != 0,
            comment: db.notes,
            fx_rate: db.fx_rate,
            created_at: db.created_at,
            updated_at: db.updated_at,
            account_name: db.account_name,
            account_currency: db.account_currency,
            asset_symbol: db.asset_symbol.unwrap_or_default(),
            asset_name: db.asset_name,
            exchange_mic: db.exchange_mic,
            asset_pricing_mode: db
                .asset_pricing_mode
                .unwrap_or_else(|| "MARKET".to_string()),
            instrument_type: db.instrument_type,
            source_system: db.source_system,
            source_record_id: db.source_record_id,
            source_group_id: db.source_group_id,
            idempotency_key: db.idempotency_key,
            import_run_id: db.import_run_id,
            is_user_modified: db.is_user_modified != 0,
            metadata: db.metadata.and_then(|s| serde_json::from_str(&s).ok()),
        }
    }
}

impl From<ImportTemplateDB> for mizan_core::activities::ImportTemplate {
    fn from(db: ImportTemplateDB) -> Self {
        use mizan_core::activities::{ImportTemplateScope, TemplateKind};

        let scope = match db.scope.as_str() {
            "SYSTEM" => ImportTemplateScope::System,
            _ => ImportTemplateScope::User,
        };

        let kind = match db.kind.as_str() {
            "CSV_HOLDINGS" => TemplateKind::CsvHoldings,
            "BROKER_ACTIVITY" => TemplateKind::BrokerActivity,
            _ => TemplateKind::CsvActivity,
        };

        Self {
            id: db.id,
            name: db.name,
            scope,
            kind,
            source_system: db.source_system,
            config_version: db.config_version,
            config: db.config,
            created_at: db.created_at,
            updated_at: db.updated_at,
        }
    }
}

impl From<mizan_core::activities::ImportTemplate> for ImportTemplateDB {
    fn from(domain: mizan_core::activities::ImportTemplate) -> Self {
        let scope = match domain.scope {
            mizan_core::activities::ImportTemplateScope::System => "SYSTEM",
            mizan_core::activities::ImportTemplateScope::User => "USER",
        };

        Self {
            id: domain.id,
            name: domain.name,
            scope: scope.to_string(),
            kind: domain.kind.as_str().to_string(),
            source_system: domain.source_system,
            config_version: domain.config_version,
            config: domain.config,
            created_at: domain.created_at,
            updated_at: domain.updated_at,
        }
    }
}

// Conversion implementations

impl From<ActivityDB> for Activity {
    fn from(db: ActivityDB) -> Self {
        use chrono::DateTime;

        // Parse status string to ActivityStatus enum
        let status = match db.status.as_str() {
            "POSTED" => ActivityStatus::Posted,
            "PENDING" => ActivityStatus::Pending,
            "DRAFT" => ActivityStatus::Draft,
            "VOID" => ActivityStatus::Void,
            _ => ActivityStatus::Posted, // Default to Posted for unknown values
        };

        // Parse metadata JSON if present
        let metadata = db
            .metadata
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok());

        Self {
            id: db.id,
            account_id: db.account_id,
            asset_id: db.asset_id,

            // Classification
            activity_type: db.activity_type,
            activity_type_override: db.activity_type_override,
            source_type: db.source_type,
            subtype: db.subtype,
            status,

            // Timing — accept both RFC3339 ("2025-03-20T00:00:00Z") AND
            // bare ISO date ("2025-03-20"). The latter is what hand-
            // crafted seed rows and the WRITE path's tolerant fallback
            // produce. Without the bare-date branch, the loader was
            // silently rewriting every unparseable activity_date to
            // Utc::now() — which made SPLIT corporate actions land on
            // *today* instead of their real date, so the split-factor
            // ordering never fired ("split date > BUY date" is false
            // when both got rewritten to today). Critical date-domain
            // data-corruption bug surfaced during QA Pass 2.
            activity_date: DateTime::parse_from_rfc3339(&db.activity_date)
                .map(|dt| dt.with_timezone(&Utc))
                .or_else(|rfc3339_err| {
                    NaiveDate::parse_from_str(&db.activity_date, "%Y-%m-%d")
                        .map(|d| {
                            d.and_hms_opt(0, 0, 0)
                                .expect("00:00:00 is a valid time-of-day")
                                .and_utc()
                        })
                        .map_err(|_| rfc3339_err)
                })
                .unwrap_or_else(|e| {
                    log::error!(
                        "Cannot parse activity_date '{}' (RFC3339 or YYYY-MM-DD): {}. \
                         Falling back to epoch — activity will appear at the start of \
                         history rather than landing silently on today's date.",
                        db.activity_date,
                        e
                    );
                    // Epoch start. Loud error + a date that sorts to the
                    // start of any range — far more obvious than the
                    // previous "silently rewrite to Utc::now()" behaviour
                    // which was directly responsible for the SPLIT bug.
                    DateTime::<Utc>::from_timestamp(0, 0)
                        .expect("epoch is a valid timestamp")
                }),
            settlement_date: db.settlement_date.as_ref().and_then(|s| {
                DateTime::parse_from_rfc3339(s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .or_else(|rfc_err| {
                        NaiveDate::parse_from_str(s, "%Y-%m-%d")
                            .map(|d| {
                                d.and_hms_opt(0, 0, 0)
                                    .expect("00:00:00 is a valid time-of-day")
                                    .and_utc()
                            })
                            .map_err(|_| rfc_err)
                    })
                    .ok()
            }),

            // Quantities
            quantity: db
                .quantity
                .as_ref()
                .map(|s| parse_decimal_string_tolerant(s, "quantity")),
            unit_price: db
                .unit_price
                .as_ref()
                .map(|s| parse_decimal_string_tolerant(s, "unit_price")),
            amount: db
                .amount
                .as_ref()
                .map(|s| parse_decimal_string_tolerant(s, "amount")),
            fee: db
                .fee
                .as_ref()
                .map(|s| parse_decimal_string_tolerant(s, "fee")),
            currency: db.currency,
            fx_rate: db
                .fx_rate
                .as_ref()
                .map(|s| parse_decimal_string_tolerant(s, "fx_rate")),

            // Metadata
            notes: db.notes,
            metadata,

            // Source identity
            source_system: db.source_system,
            source_record_id: db.source_record_id,
            source_group_id: db.source_group_id,
            idempotency_key: db.idempotency_key,
            import_run_id: db.import_run_id,

            // Sync flags
            is_user_modified: db.is_user_modified != 0,
            needs_review: db.needs_review != 0,

            // Audit — accept RFC3339 ("2026-05-25T15:16:19+00:00") AND
            // sqlite's default "YYYY-MM-DD HH:MM:SS" (space separator).
            // Live logs showed millions of "premature end of input"
            // errors against the space-separated default that sqlite's
            // `datetime('now')` produces; the previous Utc::now()
            // fallback silently flattened every old row's audit
            // timestamp to right-now.
            created_at: parse_timestamp_tolerant(&db.created_at, "created_at"),
            updated_at: parse_timestamp_tolerant(&db.updated_at, "updated_at"),
        }
    }
}

impl From<NewActivity> for ActivityDB {
    fn from(domain: NewActivity) -> Self {
        use chrono::DateTime;

        let now = Utc::now();

        // Parse the date and normalize to UTC
        let activity_datetime = DateTime::parse_from_rfc3339(&domain.activity_date)
            .map(|dt| dt.with_timezone(&Utc))
            .or_else(|_| {
                NaiveDate::parse_from_str(&domain.activity_date, "%Y-%m-%d").map(|date| {
                    // QA Pass 18: noon UTC (not midnight) so bare-date input
                    // round-trips through user TZ without drifting by 1 day
                    // for UTC-12..UTC+11. See activities_service.rs:1027 for
                    // the matching quote-creation convention.
                    Utc.from_utc_datetime(&date.and_hms_opt(12, 0, 0).unwrap_or_default())
                })
            })
            .unwrap_or_else(|e| {
                log::error!(
                    "Failed to parse activity date '{}': {}",
                    domain.activity_date,
                    e
                );
                // QA Pass 18: noon UTC fallback so the date round-trips
                // through user TZ consistently when the input is unparseable.
                Utc.from_utc_datetime(
                    &now.date_naive()
                        .and_hms_opt(12, 0, 0)
                        .unwrap_or_else(|| now.naive_utc()),
                )
            });

        // Convert ActivityStatus to string, defaulting to POSTED
        let status = domain
            .status
            .as_ref()
            .map(|s| match s {
                ActivityStatus::Posted => "POSTED",
                ActivityStatus::Pending => "PENDING",
                ActivityStatus::Draft => "DRAFT",
                ActivityStatus::Void => "VOID",
            })
            .unwrap_or("POSTED")
            .to_string();

        // Extract asset_id before consuming domain fields
        let asset_id = domain.get_symbol_id().map(|s| s.to_string());

        Self {
            id: domain.id.unwrap_or_default(),
            account_id: domain.account_id,
            asset_id,

            // Classification
            activity_type: domain.activity_type,
            activity_type_override: None,
            source_type: None,
            subtype: domain.subtype,
            status,

            // Timing
            activity_date: activity_datetime.to_rfc3339(),
            settlement_date: None,

            // Quantities
            quantity: domain.quantity.map(|d| d.to_string()),
            unit_price: domain.unit_price.map(|d| d.to_string()),
            amount: domain.amount.map(|d| d.to_string()),
            fee: domain.fee.map(|d| d.to_string()),
            currency: domain.currency,
            fx_rate: domain.fx_rate.map(|d| d.to_string()),

            // Metadata
            notes: domain.notes,
            metadata: domain.metadata,

            // Source identity
            source_system: domain.source_system.or(Some("MANUAL".to_string())),
            source_record_id: domain.source_record_id,
            source_group_id: domain.source_group_id,
            idempotency_key: domain.idempotency_key,
            import_run_id: None,

            // Sync flags
            is_user_modified: 0,
            needs_review: domain.needs_review.map(|b| b as i32).unwrap_or(0),

            // Audit
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
        }
    }
}

impl From<ActivityUpdate> for ActivityDB {
    fn from(domain: ActivityUpdate) -> Self {
        use chrono::DateTime;

        let now = Utc::now();

        let activity_datetime = DateTime::parse_from_rfc3339(&domain.activity_date)
            .map(|dt| dt.with_timezone(&Utc))
            .or_else(|_| {
                NaiveDate::parse_from_str(&domain.activity_date, "%Y-%m-%d").map(|date| {
                    // QA Pass 18: noon UTC (not midnight) so bare-date input
                    // round-trips through user TZ without drifting by 1 day
                    // for UTC-12..UTC+11. See activities_service.rs:1027 for
                    // the matching quote-creation convention.
                    Utc.from_utc_datetime(&date.and_hms_opt(12, 0, 0).unwrap_or_default())
                })
            })
            .unwrap_or_else(|e| {
                log::error!(
                    "Failed to parse activity date '{}': {}",
                    domain.activity_date,
                    e
                );
                // QA Pass 18: noon UTC fallback so the date round-trips
                // through user TZ consistently when the input is unparseable.
                Utc.from_utc_datetime(
                    &now.date_naive()
                        .and_hms_opt(12, 0, 0)
                        .unwrap_or_else(|| now.naive_utc()),
                )
            });

        // Convert ActivityStatus to string, defaulting to POSTED
        let status = domain
            .status
            .as_ref()
            .map(|s| match s {
                ActivityStatus::Posted => "POSTED",
                ActivityStatus::Pending => "PENDING",
                ActivityStatus::Draft => "DRAFT",
                ActivityStatus::Void => "VOID",
            })
            .unwrap_or("POSTED")
            .to_string();

        // Extract asset_id before consuming domain fields
        let asset_id = domain.get_symbol_id().map(|s| s.to_string());

        Self {
            id: domain.id,
            account_id: domain.account_id,
            asset_id,

            // Classification
            activity_type: domain.activity_type,
            activity_type_override: None,
            source_type: None,
            subtype: domain.subtype,
            status,

            // Timing
            activity_date: activity_datetime.to_rfc3339(),
            settlement_date: None,

            // Quantities
            quantity: domain.quantity.flatten().map(|d| d.to_string()),
            unit_price: domain.unit_price.flatten().map(|d| d.to_string()),
            amount: domain.amount.flatten().map(|d| d.to_string()),
            fee: domain.fee.flatten().map(|d| d.to_string()),
            currency: domain.currency,
            fx_rate: domain.fx_rate.flatten().map(|d| d.to_string()),

            // Metadata
            notes: domain.notes,
            metadata: domain.metadata,

            // Source identity - these will be preserved from existing record in repository
            source_system: None,
            source_record_id: None,
            source_group_id: None,
            idempotency_key: None,
            import_run_id: None,

            // Sync flags - mark as user modified since this is an update
            is_user_modified: 1,
            needs_review: 0,

            // Audit
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
        }
    }
}

impl From<ActivityUpsert> for ActivityDB {
    fn from(domain: ActivityUpsert) -> Self {
        use chrono::DateTime;

        let now = Utc::now();

        // Parse the date and normalize to UTC
        let activity_datetime = DateTime::parse_from_rfc3339(&domain.activity_date)
            .map(|dt| dt.with_timezone(&Utc))
            .or_else(|_| {
                NaiveDate::parse_from_str(&domain.activity_date, "%Y-%m-%d").map(|date| {
                    // QA Pass 18: noon UTC (not midnight) so bare-date input
                    // round-trips through user TZ without drifting by 1 day
                    // for UTC-12..UTC+11. See activities_service.rs:1027 for
                    // the matching quote-creation convention.
                    Utc.from_utc_datetime(&date.and_hms_opt(12, 0, 0).unwrap_or_default())
                })
            })
            .unwrap_or_else(|e| {
                log::error!(
                    "Failed to parse activity date '{}': {}",
                    domain.activity_date,
                    e
                );
                // QA Pass 18: noon UTC fallback so the date round-trips
                // through user TZ consistently when the input is unparseable.
                Utc.from_utc_datetime(
                    &now.date_naive()
                        .and_hms_opt(12, 0, 0)
                        .unwrap_or_else(|| now.naive_utc()),
                )
            });

        // Convert ActivityStatus to string, defaulting to POSTED
        let status = domain
            .status
            .as_ref()
            .map(|s| match s {
                ActivityStatus::Posted => "POSTED",
                ActivityStatus::Pending => "PENDING",
                ActivityStatus::Draft => "DRAFT",
                ActivityStatus::Void => "VOID",
            })
            .unwrap_or("POSTED")
            .to_string();

        Self {
            id: domain.id,
            account_id: domain.account_id,
            asset_id: domain.asset_id,

            // Classification
            activity_type: domain.activity_type,
            activity_type_override: None,
            source_type: None,
            subtype: domain.subtype,
            status,

            // Timing
            activity_date: activity_datetime.to_rfc3339(),
            settlement_date: None,

            // Quantities
            quantity: domain.quantity.map(|d| d.to_string()),
            unit_price: domain.unit_price.map(|d| d.to_string()),
            amount: domain.amount.map(|d| d.to_string()),
            fee: domain.fee.map(|d| d.to_string()),
            currency: domain.currency,
            fx_rate: domain.fx_rate.map(|d| d.to_string()),

            // Metadata
            notes: domain.notes,
            metadata: domain.metadata,

            // Source identity
            source_system: domain.source_system,
            source_record_id: domain.source_record_id,
            source_group_id: domain.source_group_id,
            idempotency_key: domain.idempotency_key,
            import_run_id: domain.import_run_id,

            // Sync flags - sync activities are not user modified by default
            is_user_modified: 0,
            needs_review: domain.needs_review.map(|b| b as i32).unwrap_or(0),

            // Audit
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
        }
    }
}

#[cfg(test)]
mod write_path_timezone_tests {
    //! QA Pass 18 regression. Verify that bare-date activity inputs are
    //! anchored at noon UTC, not midnight, so a date entered by a user in
    //! a Western timezone (e.g. UTC-8 Los Angeles) round-trips back to the
    //! same calendar date when displayed in their local timezone. The
    //! pre-fix midnight-UTC behaviour was a real off-by-one for the
    //! Americas: "2026-05-26" stored as 2026-05-26T00:00:00 UTC =
    //! 2026-05-25T16:00 LA local → display surfaces 2026-05-25.
    use super::*;
    use chrono::Timelike;
    use mizan_core::activities::{ActivityStatus, ActivityUpdate, ActivityUpsert, NewActivity};

    fn bare_date_new_activity() -> NewActivity {
        NewActivity {
            id: Some("test-activity".to_string()),
            account_id: "acc-1".to_string(),
            asset: None,
            activity_type: "DEPOSIT".to_string(),
            subtype: None,
            activity_date: "2026-05-26".to_string(),
            quantity: None,
            unit_price: None,
            currency: "USD".to_string(),
            fee: None,
            amount: Some(rust_decimal_macros::dec!(100)),
            status: Some(ActivityStatus::Posted),
            notes: None,
            fx_rate: None,
            metadata: None,
            needs_review: None,
            source_system: None,
            source_record_id: None,
            source_group_id: None,
            idempotency_key: None,
        }
    }

    #[test]
    fn new_activity_bare_date_is_anchored_at_noon_utc() {
        let db: ActivityDB = bare_date_new_activity().into();
        let ts =
            chrono::DateTime::parse_from_rfc3339(&db.activity_date).expect("RFC3339 round-trip");
        assert_eq!(ts.with_timezone(&Utc).date_naive().to_string(), "2026-05-26");
        assert_eq!(
            ts.with_timezone(&Utc).hour(),
            12,
            "bare-date input must store at noon UTC, not midnight — see QA Pass 18"
        );
    }

    #[test]
    fn update_bare_date_is_anchored_at_noon_utc() {
        let update = ActivityUpdate {
            id: "test-activity".to_string(),
            account_id: "acc-1".to_string(),
            asset: None,
            activity_type: "DEPOSIT".to_string(),
            subtype: None,
            activity_date: "2026-05-26".to_string(),
            quantity: None,
            unit_price: None,
            currency: "USD".to_string(),
            fee: None,
            amount: Some(Some(rust_decimal_macros::dec!(100))),
            status: Some(ActivityStatus::Posted),
            notes: None,
            fx_rate: None,
            metadata: None,
        };
        let db: ActivityDB = update.into();
        let ts = chrono::DateTime::parse_from_rfc3339(&db.activity_date).unwrap();
        assert_eq!(ts.with_timezone(&Utc).hour(), 12);
    }

    #[test]
    fn upsert_bare_date_is_anchored_at_noon_utc() {
        let upsert = ActivityUpsert {
            id: "test-activity".to_string(),
            account_id: "acc-1".to_string(),
            asset_id: None,
            activity_type: "DEPOSIT".to_string(),
            subtype: None,
            activity_date: "2026-05-26".to_string(),
            quantity: None,
            unit_price: None,
            currency: "USD".to_string(),
            fee: None,
            amount: Some(rust_decimal_macros::dec!(100)),
            status: Some(ActivityStatus::Posted),
            notes: None,
            fx_rate: None,
            metadata: None,
            needs_review: None,
            source_system: None,
            source_record_id: None,
            source_group_id: None,
            idempotency_key: None,
            import_run_id: None,
        };
        let db: ActivityDB = upsert.into();
        let ts = chrono::DateTime::parse_from_rfc3339(&db.activity_date).unwrap();
        assert_eq!(ts.with_timezone(&Utc).hour(), 12);
    }

    /// Cross-timezone proof: noon UTC round-trips to the same calendar
    /// date for every fixed offset from UTC-11 to UTC+11. (We use
    /// FixedOffset rather than IANA names to keep this crate dep-free of
    /// chrono-tz; the IANA equivalent is verified at the time_utils layer.)
    #[test]
    fn noon_utc_round_trips_to_same_date_at_all_common_offsets() {
        use chrono::FixedOffset;
        let db: ActivityDB = bare_date_new_activity().into();
        let utc_ts =
            chrono::DateTime::parse_from_rfc3339(&db.activity_date).unwrap().with_timezone(&Utc);
        // Offsets in hours from UTC, covering the worst-case Western and
        // Eastern zones the pre-fix bug would have broken.
        let offsets_hours: &[i32] = &[
            -11, // Pacific/Pago_Pago
            -8,  // America/Los_Angeles standard
            -5,  // America/New_York standard
            0,   // London / UTC
            1,   // Europe/Berlin
            5,   // Asia/Karachi
            9,   // Asia/Tokyo
            11,  // Australia/Sydney DST
        ];
        for h in offsets_hours {
            let tz = if *h >= 0 {
                FixedOffset::east_opt(h * 3600).unwrap()
            } else {
                FixedOffset::west_opt((-h) * 3600).unwrap()
            };
            let local_date = utc_ts.with_timezone(&tz).date_naive();
            assert_eq!(
                local_date.to_string(),
                "2026-05-26",
                "noon UTC must surface as 2026-05-26 at offset {:+}h (pre-Pass-18 \
                 midnight-UTC input drifted to 2026-05-25 for Western offsets)",
                h
            );
        }
    }
}
