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

use crate::{app::App, routes, state::AppState};

impl axum::extract::FromRef<AppState> for LeptosOptions {
    fn from_ref(state: &AppState) -> Self {
        state.leptos_options().clone()
    }
}

/// Build the full application router.
pub fn build(state: AppState) -> Router {
    use crate::app::Shell;

    let leptos_options = state.leptos_options().clone();
    let routes = generate_route_list(App);

    let api = Router::new()
        .nest("/auth", routes::auth::router())
        .nest("/qr", routes::qr::router())
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
        .layer(RequestBodyLimitLayer::new(16 * 1024 * 1024))
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
