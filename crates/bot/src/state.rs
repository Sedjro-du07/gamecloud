//! Shared state passed to every Poise command.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

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

/// How long after the bot changes a member's roles its own
/// `GuildMemberUpdate` echoes should be ignored.
const ECHO_WINDOW: Duration = Duration::from_secs(20);

struct Inner {
    config: Config,
    pool: PgPool,
    http: reqwest::Client,
    /// Members whose roles the bot is currently rewriting.
    suppressed: Mutex<HashMap<u64, Instant>>,
}

impl BotState {
    /// Build a new state.
    pub fn new(config: Config, pool: PgPool) -> Self {
        Self {
            inner: Arc::new(Inner {
                config,
                pool,
                http: reqwest::Client::new(),
                suppressed: Mutex::new(HashMap::new()),
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

    /// Mark a member as being rewritten by the bot itself.
    ///
    /// Adding eight track roles produces eight `GuildMemberUpdate`
    /// events, each carrying an *incomplete* snapshot of the member's
    /// roles. Without this, the event handler reads those snapshots as
    /// deliberate removals and the platform flaps — every track marked
    /// left, then rejoined, once per event.
    pub fn suppress_echo(&self, user_id: u64) {
        if let Ok(mut map) = self.inner.suppressed.lock() {
            map.insert(user_id, Instant::now());
        }
    }

    /// Whether a `GuildMemberUpdate` for this member is the bot's own
    /// echo and should be ignored.
    ///
    /// Expired entries are dropped on the way past, which keeps the map
    /// bounded without a sweeper task.
    pub fn is_echo(&self, user_id: u64) -> bool {
        let Ok(mut map) = self.inner.suppressed.lock() else {
            return false;
        };
        map.retain(|_, at| at.elapsed() < ECHO_WINDOW);
        map.contains_key(&user_id)
    }
}

/// Poise context type alias.
pub type Context<'a> = poise::Context<'a, BotState, anyhow::Error>;
