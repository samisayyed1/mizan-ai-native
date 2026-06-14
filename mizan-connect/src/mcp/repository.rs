//! DB queries for the MCP gateway tables.

use sqlx::{PgPool, Postgres, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct McpServerRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub source: String,
    pub display_name: String,
    pub custom_server_url: Option<String>,
    pub trust_level: String,
    pub user_acknowledged_at: Option<OffsetDateTime>,
    pub disconnected_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct McpServerInsert<'a> {
    pub user_id: Uuid,
    pub source: &'a str, // "catalog" or "self_registered"
    pub catalog_entry_id: Option<Uuid>,
    pub custom_server_url: Option<&'a str>,
    pub auth_method: Option<&'a str>,
    pub display_name: &'a str,
    pub encrypted_credentials: Option<&'a [u8]>,
    pub credentials_nonce: Option<&'a [u8]>,
    pub trust_level: &'a str, // "untrusted" or "trusted"
    pub user_acknowledged: bool,
}

/// Insert a new MCP server registration. Self-registered servers
/// require `user_acknowledged` per spec §21.5 — the caller is
/// responsible for enforcing this; we just persist the timestamp.
pub async fn insert_server(
    pool: &PgPool,
    insert: &McpServerInsert<'_>,
) -> Result<Uuid, sqlx::Error> {
    let acknowledged_at: Option<OffsetDateTime> =
        insert.user_acknowledged.then(OffsetDateTime::now_utc);

    let (id,): (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO mcp_servers (
            user_id, source, catalog_entry_id,
            custom_server_url, auth_method, display_name,
            encrypted_credentials, credentials_nonce,
            trust_level, user_acknowledged_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING id
        "#,
    )
    .bind(insert.user_id)
    .bind(insert.source)
    .bind(insert.catalog_entry_id)
    .bind(insert.custom_server_url)
    .bind(insert.auth_method)
    .bind(insert.display_name)
    .bind(insert.encrypted_credentials)
    .bind(insert.credentials_nonce)
    .bind(insert.trust_level)
    .bind(acknowledged_at)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// List the user's active (non-disconnected) MCP servers.
pub async fn list_active_servers(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<McpServerRow>, sqlx::Error> {
    let rows: Vec<(
        Uuid,
        Uuid,
        String,
        String,
        Option<String>,
        String,
        Option<OffsetDateTime>,
        Option<OffsetDateTime>,
        OffsetDateTime,
    )> = sqlx::query_as(
        r#"
        SELECT
            id, user_id, source, display_name, custom_server_url,
            trust_level, user_acknowledged_at, disconnected_at, created_at
        FROM mcp_servers
        WHERE user_id = $1 AND disconnected_at IS NULL
        ORDER BY created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                user_id,
                source,
                display_name,
                custom_server_url,
                trust_level,
                user_acknowledged_at,
                disconnected_at,
                created_at,
            )| {
                McpServerRow {
                    id,
                    user_id,
                    source,
                    display_name,
                    custom_server_url,
                    trust_level,
                    user_acknowledged_at,
                    disconnected_at,
                    created_at,
                }
            },
        )
        .collect())
}

/// Look up a single server scoped to the requesting user. Returns
/// `None` if the row doesn't belong to the user or is already
/// disconnected.
pub async fn fetch_active_server(
    pool: &PgPool,
    user_id: Uuid,
    server_id: Uuid,
) -> Result<Option<McpServerRow>, sqlx::Error> {
    let row: Option<(
        Uuid,
        Uuid,
        String,
        String,
        Option<String>,
        String,
        Option<OffsetDateTime>,
        Option<OffsetDateTime>,
        OffsetDateTime,
    )> = sqlx::query_as(
        r#"
        SELECT
            id, user_id, source, display_name, custom_server_url,
            trust_level, user_acknowledged_at, disconnected_at, created_at
        FROM mcp_servers
        WHERE id = $1 AND user_id = $2 AND disconnected_at IS NULL
        LIMIT 1
        "#,
    )
    .bind(server_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(
        |(
            id,
            user_id,
            source,
            display_name,
            custom_server_url,
            trust_level,
            user_acknowledged_at,
            disconnected_at,
            created_at,
        )| {
            McpServerRow {
                id,
                user_id,
                source,
                display_name,
                custom_server_url,
                trust_level,
                user_acknowledged_at,
                disconnected_at,
                created_at,
            }
        },
    ))
}

/// Mark a server as disconnected. Returns the number of rows affected
/// (0 if nothing matched — bad id or already disconnected).
pub async fn disconnect_server(
    pool: &PgPool,
    user_id: Uuid,
    server_id: Uuid,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE mcp_servers
        SET disconnected_at = NOW()
        WHERE id = $1 AND user_id = $2 AND disconnected_at IS NULL
        "#,
    )
    .bind(server_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// Append a row to `mcp_call_log`. Per migration 0013 + working-
/// agreement §3.4: only digests are stored, never the raw payload.
#[allow(clippy::too_many_arguments)]
pub async fn record_call(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    server_id: Uuid,
    tool_name: &str,
    params_digest: &str,
    response_digest: &str,
    duration_ms: i32,
    outcome: &str, // "ok" / "error" / "rejected_by_dlp" / "rejected_by_sandbox_gate" / "timeout"
    error_message: Option<&str>,
) -> Result<Uuid, sqlx::Error> {
    let (id,): (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO mcp_call_log (
            user_id, server_id, tool_name,
            params_digest, response_digest,
            duration_ms, outcome, error_message
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(server_id)
    .bind(tool_name)
    .bind(params_digest)
    .bind(response_digest)
    .bind(duration_ms)
    .bind(outcome)
    .bind(error_message)
    .fetch_one(&mut **tx)
    .await?;
    Ok(id)
}

/// Count calls for a user in the last N seconds. Used for the per-
/// user / per-trust-level rate limiter (spec §21.4).
pub async fn count_calls_in_window(
    pool: &PgPool,
    user_id: Uuid,
    window_secs: i64,
) -> Result<i64, sqlx::Error> {
    let (count,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)::bigint
        FROM mcp_call_log
        WHERE user_id = $1
          AND created_at > NOW() - make_interval(secs => $2)
        "#,
    )
    .bind(user_id)
    .bind(window_secs)
    .fetch_one(pool)
    .await?;
    Ok(count)
}
