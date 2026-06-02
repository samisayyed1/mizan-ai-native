use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::AppError;

use super::types::{
    PlaidAccount, PlaidAccountDto, PlaidConnectionDto, PlaidInvestmentTransaction,
    PlaidInvestmentTransactionDto, PlaidTransaction, RemovedTransaction, StoredPlaidItem,
    UpsertPlaidItem,
};

pub async fn upsert_item(pool: &PgPool, item: UpsertPlaidItem<'_>) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO plaid_items (
            user_id, item_id, access_token_encrypted, institution_id, institution_name, status
        )
        VALUES ($1, $2, $3, $4, $5, 'connected')
        ON CONFLICT (user_id, item_id)
        DO UPDATE SET
            access_token_encrypted = EXCLUDED.access_token_encrypted,
            institution_id = EXCLUDED.institution_id,
            institution_name = EXCLUDED.institution_name,
            status = 'connected',
            last_error = NULL,
            updated_at = NOW()
        "#,
    )
    .bind(item.user_id)
    .bind(item.item_id)
    .bind(item.access_token_encrypted)
    .bind(item.institution_id)
    .bind(item.institution_name)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn fetch_item(
    pool: &PgPool,
    user_id: Uuid,
    item_id: &str,
) -> Result<StoredPlaidItem, AppError> {
    let row = sqlx::query_as::<_, StoredPlaidItem>(
        r#"
        SELECT item_id, access_token_encrypted
        FROM plaid_items
        WHERE user_id = $1 AND item_id = $2 AND status <> 'disconnected'
        "#,
    )
    .bind(user_id)
    .bind(item_id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn fetch_items(pool: &PgPool, user_id: Uuid) -> Result<Vec<StoredPlaidItem>, AppError> {
    let rows = sqlx::query_as::<_, StoredPlaidItem>(
        r#"
        SELECT item_id, access_token_encrypted
        FROM plaid_items
        WHERE user_id = $1 AND status <> 'disconnected'
        ORDER BY updated_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_connections(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<PlaidConnectionDto>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT
            i.item_id,
            i.institution_id,
            i.institution_name,
            i.status,
            i.last_successful_sync_at,
            i.last_error,
            i.updated_at,
            COUNT(a.id)::BIGINT AS account_count
        FROM plaid_items i
        LEFT JOIN plaid_accounts a
          ON a.user_id = i.user_id AND a.item_id = i.item_id
        WHERE i.user_id = $1 AND i.status <> 'disconnected'
        GROUP BY i.id
        ORDER BY i.updated_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok(PlaidConnectionDto {
                item_id: row.try_get("item_id")?,
                institution_id: row.try_get("institution_id")?,
                institution_name: row.try_get("institution_name")?,
                status: row.try_get("status")?,
                account_count: row.try_get("account_count")?,
                last_successful_sync_at: row.try_get("last_successful_sync_at")?,
                last_error: row.try_get("last_error")?,
                updated_at: row.try_get("updated_at")?,
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()
        .map_err(AppError::from)
}

pub async fn list_accounts(pool: &PgPool, user_id: Uuid) -> Result<Vec<PlaidAccountDto>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT
            a.account_id,
            a.item_id,
            a.name,
            a.official_name,
            i.institution_name,
            a.account_type,
            a.subtype,
            a.mask,
            a.balances_json,
            a.updated_at
        FROM plaid_accounts a
        LEFT JOIN plaid_items i
          ON i.user_id = a.user_id AND i.item_id = a.item_id
        WHERE a.user_id = $1
          AND COALESCE(i.status, 'connected') <> 'disconnected'
        ORDER BY i.institution_name NULLS LAST, a.name NULLS LAST, a.updated_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok(PlaidAccountDto {
                account_id: row.try_get("account_id")?,
                item_id: row.try_get("item_id")?,
                name: row.try_get("name")?,
                official_name: row.try_get("official_name")?,
                institution_name: row.try_get("institution_name")?,
                account_type: row.try_get("account_type")?,
                subtype: row.try_get("subtype")?,
                mask: row.try_get("mask")?,
                balances: row.try_get("balances_json")?,
                updated_at: row.try_get("updated_at")?,
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()
        .map_err(AppError::from)
}

pub async fn disconnect_item(pool: &PgPool, user_id: Uuid, item_id: &str) -> Result<(), AppError> {
    let result = sqlx::query(
        r#"
        UPDATE plaid_items
           SET status = 'disconnected',
               last_error = NULL,
               updated_at = NOW()
         WHERE user_id = $1 AND item_id = $2 AND status <> 'disconnected'
        "#,
    )
    .bind(user_id)
    .bind(item_id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::not_found("Plaid connection was not found"));
    }

    Ok(())
}

pub async fn upsert_accounts(
    pool: &PgPool,
    user_id: Uuid,
    item_id: &str,
    accounts: &[PlaidAccount],
) -> Result<usize, AppError> {
    for account in accounts {
        sqlx::query(
            r#"
            INSERT INTO plaid_accounts (
                user_id, item_id, account_id, name, official_name, account_type,
                subtype, mask, balances_json, raw_json
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (user_id, account_id)
            DO UPDATE SET
                item_id = EXCLUDED.item_id,
                name = EXCLUDED.name,
                official_name = EXCLUDED.official_name,
                account_type = EXCLUDED.account_type,
                subtype = EXCLUDED.subtype,
                mask = EXCLUDED.mask,
                balances_json = EXCLUDED.balances_json,
                raw_json = EXCLUDED.raw_json,
                updated_at = NOW()
            "#,
        )
        .bind(user_id)
        .bind(item_id)
        .bind(&account.account_id)
        .bind(&account.name)
        .bind(&account.official_name)
        .bind(&account.account_type)
        .bind(&account.subtype)
        .bind(&account.mask)
        .bind(&account.balances)
        .bind(serde_json::to_value(account).unwrap_or(serde_json::Value::Null))
        .execute(pool)
        .await?;
    }
    Ok(accounts.len())
}

pub async fn transaction_cursor(
    pool: &PgPool,
    user_id: Uuid,
    item_id: &str,
) -> Result<Option<String>, AppError> {
    let cursor = sqlx::query_scalar::<_, Option<String>>(
        "SELECT transactions_cursor FROM plaid_items WHERE user_id = $1 AND item_id = $2",
    )
    .bind(user_id)
    .bind(item_id)
    .fetch_optional(pool)
    .await?
    .flatten();
    Ok(cursor)
}

pub async fn store_transactions(
    pool: &PgPool,
    user_id: Uuid,
    item_id: &str,
    added: &[PlaidTransaction],
    modified: &[PlaidTransaction],
    removed: &[RemovedTransaction],
    next_cursor: &str,
) -> Result<(), AppError> {
    for txn in added.iter().chain(modified.iter()) {
        sqlx::query(
            r#"
            INSERT INTO plaid_transactions (
                user_id, item_id, account_id, transaction_id, amount, iso_currency_code,
                merchant_name, name, category_json, date, pending, raw_json, removed
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10::DATE, $11, $12, FALSE)
            ON CONFLICT (user_id, transaction_id)
            DO UPDATE SET
                item_id = EXCLUDED.item_id,
                account_id = EXCLUDED.account_id,
                amount = EXCLUDED.amount,
                iso_currency_code = EXCLUDED.iso_currency_code,
                merchant_name = EXCLUDED.merchant_name,
                name = EXCLUDED.name,
                category_json = EXCLUDED.category_json,
                date = EXCLUDED.date,
                pending = EXCLUDED.pending,
                raw_json = EXCLUDED.raw_json,
                removed = FALSE,
                updated_at = NOW()
            "#,
        )
        .bind(user_id)
        .bind(item_id)
        .bind(&txn.account_id)
        .bind(&txn.transaction_id)
        .bind(txn.amount)
        .bind(
            txn.iso_currency_code
                .as_ref()
                .or(txn.currency_code.as_ref()),
        )
        .bind(&txn.merchant_name)
        .bind(&txn.name)
        .bind(serde_json::to_value(&txn.category).unwrap_or(serde_json::Value::Null))
        .bind(&txn.date)
        .bind(txn.pending.unwrap_or(false))
        .bind(serde_json::to_value(txn).unwrap_or(serde_json::Value::Null))
        .execute(pool)
        .await?;
    }

    for txn in removed {
        sqlx::query(
            r#"
            UPDATE plaid_transactions
               SET removed = TRUE, updated_at = NOW()
             WHERE user_id = $1 AND transaction_id = $2
            "#,
        )
        .bind(user_id)
        .bind(&txn.transaction_id)
        .execute(pool)
        .await?;
    }

    sqlx::query(
        r#"
        UPDATE plaid_items
           SET transactions_cursor = $3,
               last_successful_sync_at = NOW(),
               last_error = NULL,
               updated_at = NOW()
         WHERE user_id = $1 AND item_id = $2
        "#,
    )
    .bind(user_id)
    .bind(item_id)
    .bind(next_cursor)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn store_liabilities(
    pool: &PgPool,
    user_id: Uuid,
    item_id: &str,
    liabilities: &serde_json::Value,
) -> Result<usize, AppError> {
    let count = liabilities
        .as_object()
        .map(|obj| {
            obj.values()
                .filter_map(|v| v.as_array())
                .map(Vec::len)
                .sum()
        })
        .unwrap_or(0);
    sqlx::query(
        r#"
        INSERT INTO plaid_liabilities (user_id, item_id, liabilities_json)
        VALUES ($1, $2, $3)
        ON CONFLICT (user_id, item_id)
        DO UPDATE SET liabilities_json = EXCLUDED.liabilities_json, updated_at = NOW()
        "#,
    )
    .bind(user_id)
    .bind(item_id)
    .bind(liabilities)
    .execute(pool)
    .await?;
    Ok(count)
}

pub async fn replace_holdings(
    pool: &PgPool,
    user_id: Uuid,
    item_id: &str,
    holdings: &[serde_json::Value],
    securities: &[serde_json::Value],
) -> Result<usize, AppError> {
    sqlx::query("DELETE FROM plaid_investment_holdings WHERE user_id = $1 AND item_id = $2")
        .bind(user_id)
        .bind(item_id)
        .execute(pool)
        .await?;

    for holding in holdings {
        let account_id = holding
            .get("account_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let security_id = holding
            .get("security_id")
            .and_then(serde_json::Value::as_str);
        sqlx::query(
            r#"
            INSERT INTO plaid_investment_holdings (
                user_id, item_id, account_id, security_id, holding_json, securities_json
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(user_id)
        .bind(item_id)
        .bind(account_id)
        .bind(security_id)
        .bind(holding)
        .bind(serde_json::to_value(securities).unwrap_or(serde_json::Value::Null))
        .execute(pool)
        .await?;
    }
    Ok(holdings.len())
}

/// Page a user's investment transactions for the desktop.
///
/// Filters:
///   - `since` (optional ISO date string) → `transaction_date >= since`.
///   - `account_id` (optional) → narrow to a single Plaid account.
///   - `limit` (capped server-side) → cap row count.
///
/// Ordered DESC by `transaction_date` so the desktop gets the most
/// recent activity first (matches the activities-timeline reading
/// pattern; older history can be paged with `since`).
pub async fn list_investment_transactions(
    pool: &PgPool,
    user_id: Uuid,
    since: Option<&str>,
    account_id: Option<&str>,
    limit: i64,
) -> Result<Vec<PlaidInvestmentTransactionDto>, AppError> {
    // Cast NUMERIC columns to TEXT in-query so Postgres does the decimal
    // formatting — avoids needing sqlx's `bigdecimal` feature flag for
    // a single read path. The desktop receives strings like "12.500000"
    // and parses them into Decimal at the boundary, matching the
    // existing Decimal-as-string convention used for activity wire data.
    let rows = sqlx::query(
        r#"
        SELECT
            investment_transaction_id,
            item_id,
            account_id,
            type,
            subtype,
            name,
            security_id,
            security_ticker_symbol,
            security_name,
            security_type,
            security_cusip,
            security_isin,
            amount::text AS amount,
            price::text AS price,
            quantity::text AS quantity,
            fees::text AS fees,
            iso_currency_code,
            unofficial_currency_code,
            to_char(transaction_date, 'YYYY-MM-DD') AS transaction_date,
            cancelled,
            raw_json,
            updated_at
        FROM plaid_investment_transactions
        WHERE user_id = $1
          AND ($2::TEXT IS NULL OR account_id = $2)
          AND ($3::DATE IS NULL OR transaction_date >= $3::DATE)
        ORDER BY transaction_date DESC, investment_transaction_id ASC
        LIMIT $4
        "#,
    )
    .bind(user_id)
    .bind(account_id)
    .bind(since)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(PlaidInvestmentTransactionDto {
            investment_transaction_id: row.get("investment_transaction_id"),
            item_id: row.get("item_id"),
            account_id: row.get("account_id"),
            transaction_type: row.get("type"),
            subtype: row.get("subtype"),
            name: row.get("name"),
            security_id: row.get("security_id"),
            security_ticker_symbol: row.get("security_ticker_symbol"),
            security_name: row.get("security_name"),
            security_type: row.get("security_type"),
            security_cusip: row.get("security_cusip"),
            security_isin: row.get("security_isin"),
            amount: row.get("amount"),
            price: row.get("price"),
            quantity: row.get("quantity"),
            fees: row.get("fees"),
            iso_currency_code: row.get("iso_currency_code"),
            unofficial_currency_code: row.get("unofficial_currency_code"),
            transaction_date: row.get("transaction_date"),
            cancelled: row.get("cancelled"),
            raw_json: row.get("raw_json"),
            updated_at: row.get("updated_at"),
        });
    }
    Ok(out)
}

/// Read the date we last successfully pulled investment transactions
/// through for this Plaid item. NULL means the item has never been
/// synced; the handler interprets that as "do an initial backfill".
pub async fn investment_transactions_sync_through(
    pool: &PgPool,
    user_id: Uuid,
    item_id: &str,
) -> Result<Option<time::OffsetDateTime>, AppError> {
    let val: Option<time::OffsetDateTime> = sqlx::query_scalar(
        r#"
        SELECT investment_transactions_sync_through
        FROM plaid_items
        WHERE user_id = $1 AND item_id = $2 AND status <> 'disconnected'
        "#,
    )
    .bind(user_id)
    .bind(item_id)
    .fetch_optional(pool)
    .await?
    .flatten();
    Ok(val)
}

/// Upsert a batch of Plaid investment transactions.
///
/// We treat `cancel_transaction_id IS NOT NULL` as the cancellation
/// signal Plaid documents — we keep the row and flip the `cancelled`
/// flag so the desktop can reconcile against the historic ledger.
///
/// `securities` carries the page's matching securities array; we use
/// it to denormalize ticker_symbol/name/type/cusip/isin onto the
/// transaction row so the desktop can resolve to a local asset in a
/// single GET.
///
/// On success, stamp `investment_transactions_sync_through` to the
/// supplied `synced_through` (typically the end_date the handler used)
/// so the next incremental sync narrows its window correctly.
pub async fn store_investment_transactions(
    pool: &PgPool,
    user_id: Uuid,
    item_id: &str,
    transactions: &[PlaidInvestmentTransaction],
    securities: &[serde_json::Value],
    synced_through: time::OffsetDateTime,
) -> Result<usize, AppError> {
    // Build a security_id → (ticker, name, type, cusip, isin) lookup so
    // each transaction's denormalized columns can be filled in O(1).
    // Done once per batch, not per transaction.
    let mut sec_index: std::collections::HashMap<
        &str,
        (
            Option<&str>,
            Option<&str>,
            Option<&str>,
            Option<&str>,
            Option<&str>,
        ),
    > = std::collections::HashMap::with_capacity(securities.len());
    for sec in securities {
        if let Some(id) = sec.get("security_id").and_then(|v| v.as_str()) {
            sec_index.insert(
                id,
                (
                    sec.get("ticker_symbol").and_then(|v| v.as_str()),
                    sec.get("name").and_then(|v| v.as_str()),
                    sec.get("type").and_then(|v| v.as_str()),
                    sec.get("cusip").and_then(|v| v.as_str()),
                    sec.get("isin").and_then(|v| v.as_str()),
                ),
            );
        }
    }

    let mut written = 0usize;
    for txn in transactions {
        let cancelled = txn.cancel_transaction_id.is_some();
        let (sec_symbol, sec_name, sec_type, sec_cusip, sec_isin) = txn
            .security_id
            .as_deref()
            .and_then(|sid| sec_index.get(sid).copied())
            .unwrap_or((None, None, None, None, None));
        // PlaidInvestmentTransaction.date is "YYYY-MM-DD"; let Postgres
        // do the cast via $N::DATE rather than parsing in Rust.
        sqlx::query(
            r#"
            INSERT INTO plaid_investment_transactions (
                user_id, item_id, account_id, investment_transaction_id, type, subtype,
                name, security_id, amount, price, quantity, fees,
                iso_currency_code, unofficial_currency_code, transaction_date,
                cancelled, raw_json,
                security_ticker_symbol, security_name, security_type,
                security_cusip, security_isin
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                $13, $14, $15::DATE, $16, $17,
                $18, $19, $20, $21, $22
            )
            ON CONFLICT (user_id, investment_transaction_id)
            DO UPDATE SET
                item_id = EXCLUDED.item_id,
                account_id = EXCLUDED.account_id,
                type = EXCLUDED.type,
                subtype = EXCLUDED.subtype,
                name = EXCLUDED.name,
                security_id = EXCLUDED.security_id,
                amount = EXCLUDED.amount,
                price = EXCLUDED.price,
                quantity = EXCLUDED.quantity,
                fees = EXCLUDED.fees,
                iso_currency_code = EXCLUDED.iso_currency_code,
                unofficial_currency_code = EXCLUDED.unofficial_currency_code,
                transaction_date = EXCLUDED.transaction_date,
                cancelled = EXCLUDED.cancelled,
                raw_json = EXCLUDED.raw_json,
                security_ticker_symbol = EXCLUDED.security_ticker_symbol,
                security_name = EXCLUDED.security_name,
                security_type = EXCLUDED.security_type,
                security_cusip = EXCLUDED.security_cusip,
                security_isin = EXCLUDED.security_isin,
                updated_at = NOW()
            "#,
        )
        .bind(user_id)
        .bind(item_id)
        .bind(&txn.account_id)
        .bind(&txn.investment_transaction_id)
        .bind(&txn.transaction_type)
        .bind(&txn.subtype)
        .bind(&txn.name)
        .bind(&txn.security_id)
        // Plaid returns these as f64; convert to PG NUMERIC via string
        // round-trip to preserve precision at the storage boundary.
        .bind(txn.amount.map(|v| v.to_string()))
        .bind(txn.price.map(|v| v.to_string()))
        .bind(txn.quantity.map(|v| v.to_string()))
        .bind(txn.fees.map(|v| v.to_string()))
        .bind(&txn.iso_currency_code)
        .bind(&txn.unofficial_currency_code)
        .bind(&txn.date)
        .bind(cancelled)
        .bind(serde_json::to_value(txn).unwrap_or(serde_json::Value::Null))
        .bind(sec_symbol)
        .bind(sec_name)
        .bind(sec_type)
        .bind(sec_cusip)
        .bind(sec_isin)
        .execute(pool)
        .await?;
        written += 1;
    }

    sqlx::query(
        r#"
        UPDATE plaid_items
           SET investment_transactions_sync_through = $3,
               last_successful_sync_at = NOW(),
               last_error = NULL,
               updated_at = NOW()
         WHERE user_id = $1 AND item_id = $2
        "#,
    )
    .bind(user_id)
    .bind(item_id)
    .bind(synced_through)
    .execute(pool)
    .await?;

    Ok(written)
}

/// Returns the prior `last_sync_attempt_at` for an item (if any), then
/// stamps the column to `NOW()`. The handler uses the prior value to decide
/// whether the request is within the cooldown window.
pub async fn record_sync_attempt(
    pool: &PgPool,
    user_id: Uuid,
    item_id: &str,
) -> Result<Option<time::OffsetDateTime>, AppError> {
    let prev: Option<Option<time::OffsetDateTime>> = sqlx::query_scalar(
        r#"
        SELECT last_sync_attempt_at
        FROM plaid_items
        WHERE user_id = $1 AND item_id = $2 AND status <> 'disconnected'
        "#,
    )
    .bind(user_id)
    .bind(item_id)
    .fetch_optional(pool)
    .await?;

    sqlx::query(
        r#"
        UPDATE plaid_items
           SET last_sync_attempt_at = NOW()
         WHERE user_id = $1 AND item_id = $2 AND status <> 'disconnected'
        "#,
    )
    .bind(user_id)
    .bind(item_id)
    .execute(pool)
    .await?;

    Ok(prev.flatten())
}

pub async fn mark_item_error(
    pool: &PgPool,
    user_id: Uuid,
    item_id: &str,
    error: &str,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        UPDATE plaid_items
           SET status = 'error', last_error = $3, updated_at = NOW()
         WHERE user_id = $1 AND item_id = $2
        "#,
    )
    .bind(user_id)
    .bind(item_id)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}
