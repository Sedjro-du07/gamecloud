//! PostgreSQL connection pool.

use std::time::Duration;

use sqlx::postgres::{PgPool, PgPoolOptions};

/// Build a pool with sensible production defaults:
/// - 5 minimum connections kept warm
/// - 32 maximum connections
/// - 30 second acquisition timeout
/// - 10 minute idle timeout (so cloud DBs that drop idle conns don't
///   surprise us)
///
/// # Errors
///
/// Returns the underlying `sqlx::Error` on connection failure.
pub async fn build(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .min_connections(5)
        .max_connections(32)
        .acquire_timeout(Duration::from_secs(30))
        .idle_timeout(Duration::from_secs(600))
        .test_before_acquire(true)
        .connect(database_url)
        .await
}

/// Run all pending migrations. Call this once at startup.
///
/// # Errors
///
/// Propagates any migration error.
pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("../../migrations").run(pool).await
}
