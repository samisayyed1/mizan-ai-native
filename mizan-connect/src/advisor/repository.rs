//! DB queries for the advisor tables.

use sqlx::PgPool;
use time::{Date, OffsetDateTime};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AdvisorLinkRow {
    pub id: Uuid,
    pub advisor_user_id: Uuid,
    pub client_user_id: Uuid,
    pub scope: String,
    pub granted_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
    pub revoked_at: Option<OffsetDateTime>,
}

/// Insert a new advisor link with the given hashed grant token.
/// Returns the row id.
#[allow(clippy::too_many_arguments)]
pub async fn insert_link(
    pool: &PgPool,
    advisor_user_id: Uuid,
    client_user_id: Uuid,
    scope: &str,
    grant_token_hash: &str,
    expires_at: OffsetDateTime,
) -> Result<Uuid, sqlx::Error> {
    let (id,): (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO advisor_links (
            advisor_user_id, client_user_id, scope, grant_token_hash, expires_at
        )
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (advisor_user_id, client_user_id) DO UPDATE SET
            scope            = EXCLUDED.scope,
            grant_token_hash = EXCLUDED.grant_token_hash,
            expires_at       = EXCLUDED.expires_at,
            revoked_at       = NULL,
            revoked_by_user_id = NULL,
            revoke_reason    = NULL,
            updated_at       = NOW()
        RETURNING id
        "#,
    )
    .bind(advisor_user_id)
    .bind(client_user_id)
    .bind(scope)
    .bind(grant_token_hash)
    .bind(expires_at)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// List the grants a client has issued. Returns all (including
/// revoked / expired) for full lifecycle visibility.
pub async fn list_grants_by_client(
    pool: &PgPool,
    client_user_id: Uuid,
) -> Result<Vec<AdvisorLinkRow>, sqlx::Error> {
    fetch_links_by("client_user_id", pool, client_user_id).await
}

/// List the active (non-revoked, non-expired) links for an advisor.
pub async fn list_active_links_for_advisor(
    pool: &PgPool,
    advisor_user_id: Uuid,
) -> Result<Vec<AdvisorLinkRow>, sqlx::Error> {
    let rows: Vec<(
        Uuid,
        Uuid,
        Uuid,
        String,
        OffsetDateTime,
        OffsetDateTime,
        Option<OffsetDateTime>,
    )> = sqlx::query_as(
        r#"
        SELECT id, advisor_user_id, client_user_id, scope, granted_at, expires_at, revoked_at
        FROM advisor_links
        WHERE advisor_user_id = $1
          AND revoked_at IS NULL
          AND expires_at > NOW()
        ORDER BY granted_at DESC
        "#,
    )
    .bind(advisor_user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(into_row).collect())
}

async fn fetch_links_by(
    col: &str,
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<AdvisorLinkRow>, sqlx::Error> {
    // Parameterised column name is fixed to one of two known values
    // — col is never user-controlled, only chosen by the caller of
    // this private helper.
    let sql = format!(
        r#"
        SELECT id, advisor_user_id, client_user_id, scope, granted_at, expires_at, revoked_at
        FROM advisor_links
        WHERE {col} = $1
        ORDER BY granted_at DESC
        "#,
    );
    let rows: Vec<(
        Uuid,
        Uuid,
        Uuid,
        String,
        OffsetDateTime,
        OffsetDateTime,
        Option<OffsetDateTime>,
    )> = sqlx::query_as(&sql).bind(user_id).fetch_all(pool).await?;
    Ok(rows.into_iter().map(into_row).collect())
}

fn into_row(
    t: (
        Uuid,
        Uuid,
        Uuid,
        String,
        OffsetDateTime,
        OffsetDateTime,
        Option<OffsetDateTime>,
    ),
) -> AdvisorLinkRow {
    AdvisorLinkRow {
        id: t.0,
        advisor_user_id: t.1,
        client_user_id: t.2,
        scope: t.3,
        granted_at: t.4,
        expires_at: t.5,
        revoked_at: t.6,
    }
}

/// Mark a link as revoked. Returns rows affected (0 if it didn't
/// belong to the revoker or was already revoked).
pub async fn revoke_link(
    pool: &PgPool,
    link_id: Uuid,
    revoker_user_id: Uuid,
    reason: Option<&str>,
) -> Result<u64, sqlx::Error> {
    let r = sqlx::query(
        r#"
        UPDATE advisor_links
        SET revoked_at = NOW(),
            revoked_by_user_id = $2,
            revoke_reason = $3,
            updated_at = NOW()
        WHERE id = $1
          AND revoked_at IS NULL
          AND (client_user_id = $2 OR advisor_user_id = $2)
        "#,
    )
    .bind(link_id)
    .bind(revoker_user_id)
    .bind(reason)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// Check whether an advisor currently has access to a specific
/// client. Returns the scope when access is live; `None` when there
/// is no link / it's revoked / it's expired.
pub async fn fetch_active_scope(
    pool: &PgPool,
    advisor_user_id: Uuid,
    client_user_id: Uuid,
) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT scope
        FROM advisor_links
        WHERE advisor_user_id = $1
          AND client_user_id = $2
          AND revoked_at IS NULL
          AND expires_at > NOW()
        LIMIT 1
        "#,
    )
    .bind(advisor_user_id)
    .bind(client_user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(scope,)| scope))
}

#[derive(Debug, Clone)]
pub struct SignOffRow {
    pub id: Uuid,
    pub advisor_user_id: Uuid,
    pub client_user_id: Uuid,
    pub holding_account_id: String,
    pub holding_symbol: String,
    pub holding_as_of_date: Date,
    pub note: Option<String>,
    pub signed_off_at: OffsetDateTime,
    pub retracted_at: Option<OffsetDateTime>,
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_sign_off(
    pool: &PgPool,
    advisor_user_id: Uuid,
    client_user_id: Uuid,
    holding_account_id: &str,
    holding_symbol: &str,
    holding_as_of_date: Date,
    note: Option<&str>,
) -> Result<Uuid, sqlx::Error> {
    let (id,): (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO advisor_sign_offs (
            advisor_user_id, client_user_id,
            holding_account_id, holding_symbol, holding_as_of_date, note
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id
        "#,
    )
    .bind(advisor_user_id)
    .bind(client_user_id)
    .bind(holding_account_id)
    .bind(holding_symbol)
    .bind(holding_as_of_date)
    .bind(note)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn retract_sign_off(
    pool: &PgPool,
    sign_off_id: Uuid,
    advisor_user_id: Uuid,
    reason: Option<&str>,
) -> Result<u64, sqlx::Error> {
    let r = sqlx::query(
        r#"
        UPDATE advisor_sign_offs
        SET retracted_at = NOW(),
            retract_reason = $3
        WHERE id = $1
          AND advisor_user_id = $2
          AND retracted_at IS NULL
        "#,
    )
    .bind(sign_off_id)
    .bind(advisor_user_id)
    .bind(reason)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

pub async fn list_sign_offs_for_client_by_advisor(
    pool: &PgPool,
    advisor_user_id: Uuid,
    client_user_id: Uuid,
) -> Result<Vec<SignOffRow>, sqlx::Error> {
    let rows: Vec<(
        Uuid,
        Uuid,
        Uuid,
        String,
        String,
        Date,
        Option<String>,
        OffsetDateTime,
        Option<OffsetDateTime>,
    )> = sqlx::query_as(
        r#"
        SELECT
            id, advisor_user_id, client_user_id,
            holding_account_id, holding_symbol, holding_as_of_date,
            note, signed_off_at, retracted_at
        FROM advisor_sign_offs
        WHERE advisor_user_id = $1
          AND client_user_id = $2
        ORDER BY signed_off_at DESC
        "#,
    )
    .bind(advisor_user_id)
    .bind(client_user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|t| SignOffRow {
            id: t.0,
            advisor_user_id: t.1,
            client_user_id: t.2,
            holding_account_id: t.3,
            holding_symbol: t.4,
            holding_as_of_date: t.5,
            note: t.6,
            signed_off_at: t.7,
            retracted_at: t.8,
        })
        .collect())
}

/// Append a per-action row to `advisor_access_log` for the §15.5
/// audit trail. Errors are best-effort: callers log and continue.
pub async fn record_access(
    pool: &PgPool,
    advisor_user_id: Uuid,
    client_user_id: Uuid,
    action: &str,
    context_json: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO advisor_access_log (
            advisor_user_id, client_user_id, action, context_json
        )
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(advisor_user_id)
    .bind(client_user_id)
    .bind(action)
    .bind(context_json)
    .execute(pool)
    .await?;
    Ok(())
}
