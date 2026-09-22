//! GameCloud OS web server entry point.
//!
//! Loads configuration, opens the database pool, runs migrations,
//! builds the Axum router, and serves on the configured address until
//! `SIGINT` / `SIGTERM`.

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::net::SocketAddr;

    use gamecloud_web::{
        config::Config,
        db::pool,
        router,
        state::AppState,
    };
    use leptos::config::get_configuration;
    use tracing_subscriber::EnvFilter;

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn,tower_http=info")))
        .json()
        .init();

    let config = Config::from_env()?;
    let pool = pool::build(&config.database_url).await?;
    pool::migrate(&pool).await?;

    let leptos_options = get_configuration(None)?.leptos_options;
    let state = AppState::new(config.clone(), pool, leptos_options);
    let app = router::build(state);

    let addr: SocketAddr = config.bind_addr.parse()?;
    tracing::info!(%addr, "gamecloud-web listening");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    // `into_make_service_with_connect_info` is what puts the peer address
    // into request extensions. Without it the rate limiter cannot
    // identify a client and lets every request through — which is
    // exactly what a smoke test caught.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    Ok(())
}

#[cfg(feature = "ssr")]
async fn shutdown_signal() {
    use tokio::signal;

    let ctrl_c = async {
        signal::ctrl_c().await.expect("install ctrl-c handler");
    };
    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}

#[cfg(not(feature = "ssr"))]
fn main() {}
