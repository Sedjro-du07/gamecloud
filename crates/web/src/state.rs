//! Application state injected into every Axum handler.

use std::sync::Arc;

use leptos::config::LeptosOptions;
use sqlx::PgPool;

use crate::{
    config::{Config, DiscordChannels},
    middleware::rate_limit::RateLimiter,
    services::github::GitHub,
};

/// State held by the Axum router. Cheap to clone (it's `Arc`-based).
#[derive(Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

struct Inner {
    pub config: Config,
    pub pool: PgPool,
    pub leptos_options: LeptosOptions,
    pub rate_limiter: RateLimiter,
}

impl AppState {
    /// Build a new state.
    #[must_use]
    pub fn new(config: Config, pool: PgPool, leptos_options: LeptosOptions) -> Self {
        Self {
            inner: Arc::new(Inner {
                config,
                pool,
                leptos_options,
                rate_limiter: RateLimiter::default(),
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


    /// Leptos options (site root, pkg dir, …).
    #[must_use]
    pub fn leptos_options(&self) -> &LeptosOptions {
        &self.inner.leptos_options
    }

    /// Shared per-IP rate limiter.
    #[must_use]
    pub fn rate_limiter(&self) -> &RateLimiter {
        &self.inner.rate_limiter
    }

    /// Where each kind of announcement is posted.
    #[must_use]
    pub fn channels(&self) -> DiscordChannels {
        self.inner.config.channels
    }

    /// GitHub client, when the organisation integration is configured.
    #[must_use]
    pub fn github(&self) -> Option<GitHub> {
        GitHub::from_config(&self.inner.config)
    }
}
