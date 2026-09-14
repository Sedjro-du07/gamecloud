//! Application state injected into every Axum handler.

use std::sync::Arc;

use leptos::config::LeptosOptions;
use sqlx::PgPool;

use crate::{config::Config, services::mailer::Mailer};

/// State held by the Axum router. Cheap to clone (it's `Arc`-based).
#[derive(Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

struct Inner {
    pub config: Config,
    pub pool: PgPool,
    pub mailer: Mailer,
    pub leptos_options: LeptosOptions,
}

impl AppState {
    /// Build a new state.
    #[must_use]
    pub fn new(config: Config, pool: PgPool, mailer: Mailer, leptos_options: LeptosOptions) -> Self {
        Self {
            inner: Arc::new(Inner {
                config,
                pool,
                mailer,
                leptos_options,
            }),
        }
    }

    /// Application config.
    #[must_use]
    pub fn config(&self) -> &Config {
        &self.inner.config
    }

    /// Database pool.
    #[must_use]
    pub fn pool(&self) -> &PgPool {
        &self.inner.pool
    }

    /// Outgoing-mail transport.
    #[must_use]
    pub fn mailer(&self) -> &Mailer {
        &self.inner.mailer
    }

    /// Leptos options (site root, pkg dir, …).
    #[must_use]
    pub fn leptos_options(&self) -> &LeptosOptions {
        &self.inner.leptos_options
    }
}
