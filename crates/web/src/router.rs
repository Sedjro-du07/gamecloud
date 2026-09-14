//! Top-level router composition and Tower middleware stack.

use axum::Router;
use leptos::config::LeptosOptions;
use leptos::prelude::provide_context;
use leptos_axum::{generate_route_list, LeptosRoutes};
use tower_http::{
    compression::CompressionLayer,
    cors::CorsLayer,
    limit::RequestBodyLimitLayer,
    trace::TraceLayer,
};

use crate::{app::App, middleware::rate_limit, routes, state::AppState};

impl axum::extract::FromRef<AppState> for LeptosOptions {
    fn from_ref(state: &AppState) -> Self {
        state.leptos_options().clone()
    }
}

/// Largest request body accepted, in bytes.
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

/// Build the CORS layer.
///
/// The pre-audit stack used `CorsLayer::permissive()`, which answers
/// every origin with `*`. That did not leak cookie-authenticated data —
/// permissive never sets `Allow-Credentials`, so browsers withhold
/// cookies cross-origin — but it is the wrong default to ship, and one
/// `.allow_credentials(true)` away from being a real hole. We mirror
/// the configured public origin instead.
fn cors(state: &AppState) -> CorsLayer {
    let origin = state.config().public_origin.clone();
    let Ok(value) = origin.parse::<axum::http::HeaderValue>() else {
        tracing::warn!(
            %origin,
            "PUBLIC_ORIGIN is not a valid header value; cross-origin requests are disabled"
        );
        return CorsLayer::new();
    };

    CorsLayer::new()
        .allow_origin(value)
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PATCH,
            axum::http::Method::DELETE,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::ACCEPT,
        ])
        .allow_credentials(true)
}

/// Build the full application router.
pub fn build(state: AppState) -> Router {
    use crate::app::Shell;

    let leptos_options = state.leptos_options().clone();
    let routes = generate_route_list(App);

    let api = Router::new()
        .nest("/auth", routes::auth::router())
        .nest("/qr", routes::qr::router())
        .nest("/users", routes::users::router())
        .nest("/projects", routes::projects::router())
        .nest("/quests", routes::quests::router())
        .nest("/resources", routes::resources::router())
        .nest("/seasons", routes::seasons::router())
        .nest("/admin", routes::admin::router())
        .merge(routes::webhooks::router());

    Router::new()
        .merge(routes::health::router())
        .nest("/api", api)
        .leptos_routes_with_context(
            &state,
            routes,
            {
                let state = state.clone();
                move || {
                    provide_context(state.clone());
                }
            },
            {
                let opts = leptos_options.clone();
                move || {
                    Shell(crate::app::ShellProps { options: opts.clone() })
                }
            },
        )
        .fallback(leptos_axum::file_and_error_handler::<AppState, _>(
            move |options: LeptosOptions| {
                Shell(crate::app::ShellProps { options })
            },
        ))
        // Rate limiting sits outermost of the application layers so a
        // flood is rejected before it touches the database.
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            rate_limit::layer,
        ))
        .layer(RequestBodyLimitLayer::new(MAX_BODY_BYTES))
        .layer(CompressionLayer::new())
        .layer(cors(&state))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
