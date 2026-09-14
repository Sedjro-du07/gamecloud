//! Audit trail.
//!
//! `audit_logs` existed from migration 0006 and the `ViewAuditLogs`
//! permission was defined in the shared crate, but nothing ever wrote a
//! row — the webhook module's doc comment even claimed it did. This is
//! the writer.
//!
//! Everything that changes another member's standing is recorded:
//! manual XP grants, bureau appointments, project releases, badge
//! awards, resource validations. Entries are never updated or deleted.

use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::WebResult;

/// One audit entry, as returned to the admin panel.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct AuditEntry {
    /// Entry id.
    pub id: Uuid,
    /// Member who acted, or `None` when the platform acted on its own.
    pub actor_id: Option<Uuid>,
    /// Display name of the actor, resolved for the UI.
    pub actor_name: Option<String>,
    /// Dotted action name, e.g. `admin.grant_xp`.
    pub action: String,
    /// Kind of entity affected.
    pub target_type: Option<String>,
    /// Identifier of the entity affected.
    pub target_id: Option<Uuid>,
    /// Free-form details.
    pub metadata: Option<Value>,
    /// When it happened.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Record an entry on a pool.
///
/// # Errors
/// Propagates database errors.
pub async fn record(
    pool: &PgPool,
    actor_id: Option<Uuid>,
    action: &str,
    target_type: Option<&str>,
    target_id: Option<Uuid>,
    metadata: Value,
) -> WebResult<()> {
    sqlx::query(
        r#"
        INSERT INTO audit_logs (actor_id, action, target_type, target_id, metadata)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(actor_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(metadata)
    .execute(pool)
    .await?;
    Ok(())
}

/// Record an entry inside an existing transaction, so the trail commits
/// atomically with the change it describes.
///
/// # Errors
/// Propagates database errors.
pub async fn record_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    actor_id: Option<Uuid>,
    action: &str,
    target_type: Option<&str>,
    target_id: Option<Uuid>,
    metadata: Value,
) -> WebResult<()> {
    sqlx::query(
        r#"
        INSERT INTO audit_logs (actor_id, action, target_type, target_id, metadata)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(actor_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(metadata)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Most recent entries, newest first.
///
/// # Errors
/// Propagates database errors.
pub async fn recent(pool: &PgPool, limit: i64) -> WebResult<Vec<AuditEntry>> {
    let rows = sqlx::query_as::<_, AuditEntry>(
        r#"
        SELECT a.id,
               a.actor_id,
               COALESCE(u.current_title, u.discord_id) AS actor_name,
               a.action,
               a.target_type,
               a.target_id,
               a.metadata,
               a.created_at
          FROM audit_logs a
          LEFT JOIN users u ON u.id = a.actor_id
         ORDER BY a.created_at DESC
         LIMIT $1
        "#,
    )
    .bind(limit.clamp(1, 500))
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
