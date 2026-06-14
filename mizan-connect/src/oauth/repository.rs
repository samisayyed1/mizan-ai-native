//! DB queries for the OAuth tables.
//!
//! Only the subset of `oauth_providers` + `user_oauth_connections` the
//! PR-J1.b handlers need. Refresh-worker queries land in PR-J1.c.

use sqlx::{PgPool, Postgres, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use super::types::Provider;

#[derive(Debug, Clone)]
pub struct ProviderRow {
    pub id: Uuid,
}

/// Look up a provider's row id by its catalog slug. Returns `None` if
/// the provider hasn't been seeded yet (would indicate the
/// 0015_oauth_providers_catalog_seed migration didn't run).
pub async fn provider_row(pool: &PgPool, provider: Provider) -> Result<Option<ProviderRow>, sqlx::Error> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM oauth_providers WHERE name = $1 LIMIT 1",
    )
    .bind(provider.slug())
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(id,)| ProviderRow { id }))
}

#[allow(clippy::too_many_arguments)]
pub struct ConnectionInsert<'a> {
    pub user_id: Uuid,
    pub provider_id: Uuid,
    pub encrypted_access_token: &'a [u8],
    pub access_token_nonce: &'a [u8],
    pub encrypted_refresh_token: Option<&'a [u8]>,
    pub refresh_token_nonce: Option<&'a [u8]>,
    pub scopes_granted: &'a str,
    pub expires_at: Option<OffsetDateTime>,
}

/// Insert (or refresh) a user's OAuth connection.
///
/// `UNIQUE (user_id, provider_id)` in migration 0012 means a user can
/// only have one active connection per provider — re-connecting
/// rotates the tokens via ON CONFLICT DO UPDATE.
pub async fn upsert_connection(
    tx: &mut Transaction<'_, Postgres>,
    insert: &ConnectionInsert<'_>,
) -> Result<Uuid, sqlx::Error> {
    // 12 months of consent window per working-agreement §20.4.
    let reconsent_due_at =
        OffsetDateTime::now_utc() + time::Duration::days(365);

    let id: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO user_oauth_connections (
            user_id, provider_id,
            encrypted_access_token, access_token_nonce,
            encrypted_refresh_token, refresh_token_nonce,
            scopes_granted, expires_at, reconsent_due_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT (user_id, provider_id) DO UPDATE SET
            encrypted_access_token  = EXCLUDED.encrypted_access_token,
            access_token_nonce      = EXCLUDED.access_token_nonce,
            encrypted_refresh_token = EXCLUDED.encrypted_refresh_token,
            refresh_token_nonce     = EXCLUDED.refresh_token_nonce,
            scopes_granted          = EXCLUDED.scopes_granted,
            expires_at              = EXCLUDED.expires_at,
            disconnected_at         = NULL,
            disconnect_reason       = NULL,
            last_reconsented_at     = NOW(),
            reconsent_due_at        = EXCLUDED.reconsent_due_at,
            updated_at              = NOW()
        RETURNING id
        "#,
    )
    .bind(insert.user_id)
    .bind(insert.provider_id)
    .bind(insert.encrypted_access_token)
    .bind(insert.access_token_nonce)
    .bind(insert.encrypted_refresh_token)
    .bind(insert.refresh_token_nonce)
    .bind(insert.scopes_granted)
    .bind(insert.expires_at)
    .bind(reconsent_due_at)
    .fetch_one(&mut **tx)
    .await?;

    Ok(id.0)
}

#[derive(Debug, Clone)]
pub struct ConnectionListRow {
    pub provider: Provider,
    pub provider_display_name: String,
    pub scopes_granted: String,
    pub connected_at: OffsetDateTime,
    pub expires_at: Option<OffsetDateTime>,
    pub reconsent_due_at: OffsetDateTime,
}

/// List the user's active (non-disconnected) connections joined with
/// the provider's display name, sorted by most-recently-updated.
pub async fn list_active_connections(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<ConnectionListRow>, sqlx::Error> {
    let rows: Vec<(String, String, String, OffsetDateTime, Option<OffsetDateTime>, OffsetDateTime)> =
        sqlx::query_as(
            r#"
            SELECT
                p.name,
                p.display_name,
                c.scopes_granted,
                c.created_at,
                c.expires_at,
                c.reconsent_due_at
            FROM user_oauth_connections c
            JOIN oauth_providers p ON p.id = c.provider_id
            WHERE c.user_id = $1 AND c.disconnected_at IS NULL
            ORDER BY c.updated_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

    let mut out = Vec::with_capacity(rows.len());
    for (name, display_name, scopes_granted, connected_at, expires_at, reconsent_due_at) in rows {
        // Defensive: if the DB has a stray row whose name isn't in
        // the in-code catalog, skip it. Realistically this only
        // happens after a downgrade.
        if let Some(provider) = Provider::parse(&name) {
            out.push(ConnectionListRow {
                provider,
                provider_display_name: display_name,
                scopes_granted,
                connected_at,
                expires_at,
                reconsent_due_at,
            });
        }
    }
    Ok(out)
}

/// Mark a user's connection as disconnected with a reason. Returns
/// the number of rows affected (0 if there was no active row).
pub async fn disconnect(
    pool: &PgPool,
    user_id: Uuid,
    provider: Provider,
    reason: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE user_oauth_connections c
        SET disconnected_at = NOW(),
            disconnect_reason = $3,
            updated_at = NOW()
        FROM oauth_providers p
        WHERE c.provider_id = p.id
          AND c.user_id = $1
          AND p.name = $2
          AND c.disconnected_at IS NULL
        "#,
    )
    .bind(user_id)
    .bind(provider.slug())
    .bind(reason)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}
