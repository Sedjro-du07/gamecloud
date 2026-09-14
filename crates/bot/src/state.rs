//! Shared state passed to every Poise command.

use std::sync::Arc;

use sqlx::PgPool;

use crate::config::Config;

/// Per-process state stored in the Poise framework's `Data`.
#[derive(Clone)]
pub struct BotState {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for BotState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BotState").finish_non_exhaustive()
    }
}

struct Inner {
    config: Config,
    pool: PgPool,
    http: reqwest::Client,
}

impl BotState {
    /// Build a new state.
    pub fn new(config: Config, pool: PgPool) -> Self {
        Self {
            inner: Arc::new(Inner {
                config,
                pool,
                http: reqwest::Client::new(),
            }),
        }
    }

    /// Bot configuration.
    pub fn config(&self) -> &Config {
        &self.inner.config
    }

    /// Database pool.
    pub fn pool(&self) -> &PgPool {
        &self.inner.pool
    }

    /// HTTP client (used to forward DraftBot events to the web binary).
    pub fn http(&self) -> &reqwest::Client {
        &self.inner.http
    }
}

/// Poise context type alias.
pub type Context<'a> = poise::Context<'a, BotState, anyhow::Error>;
