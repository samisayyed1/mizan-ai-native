use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::AppError;

use super::types::{
    PlaidAccount, PlaidAccountDto, PlaidConnectionDto, PlaidTransaction, RemovedTransaction,
    StoredPlaidItem, UpsertPlaidItem,
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
