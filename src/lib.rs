#[allow(clippy::all)]
pub mod apis;
pub mod auth;
pub mod config;
pub mod db;
pub mod domain;
pub mod error;
pub mod handlers;
pub mod merge;
#[allow(clippy::all)]
pub mod models;
pub mod sqlite_store;

use std::sync::Arc;

use axum::Router;
use axum::http::header;
use axum::middleware;
use axum::response::IntoResponse;
use axum::routing::get;

use config::ApiMode;
use db::DataStore;

pub struct AppState {
    pub store: Arc<dyn DataStore>,
    pub config: config::Config,
    pub openapi_doc: Arc<String>,
}

pub fn build_router(state: Arc<AppState>) -> Router {
    let flux_routes = Router::new()
        .route(
            "/api/v2/flux/clusters/{cluster_dns}/platform-components",
            get(handlers::platform_components::get_platform_components),
        )
        .route(
            "/api/v2/flux/clusters/{cluster_dns}/namespaces",
            get(handlers::namespaces::get_namespaces),
        )
        .route(
            "/api/v2/flux/clusters/{cluster_dns}/rolebindings",
            get(handlers::rolebindings::get_rolebindings),
        )
        .route(
            "/api/v2/flux/clusters",
            get(handlers::clusters::get_clusters),
        );

    let crud_routes = Router::new()
        .route(
            "/clusters",
            get(handlers::crud::list_clusters).post(handlers::crud::create_cluster),
        )
        .route(
            "/clusters/{id}",
            get(handlers::crud::get_cluster)
                .put(handlers::crud::put_cluster)
                .delete(handlers::crud::delete_cluster),
        )
        .route(
            "/platform_components",
            get(handlers::crud::list_platform_components)
                .post(handlers::crud::create_platform_component),
        )
        .route(
            "/platform_components/{id}",
            get(handlers::crud::get_platform_component)
                .put(handlers::crud::put_platform_component)
                .delete(handlers::crud::delete_platform_component),
        )
        .route(
            "/namespaces",
            get(handlers::crud::list_namespaces).post(handlers::crud::create_namespace),
        )
        .route(
            "/namespaces/{id}",
            get(handlers::crud::get_namespace)
                .put(handlers::crud::put_namespace)
                .delete(handlers::crud::delete_namespace),
        )
        .route(
            "/rolebindings",
            get(handlers::crud::list_rolebindings).post(handlers::crud::create_rolebinding),
        )
        .route(
            "/rolebindings/{id}",
            get(handlers::crud::get_rolebinding)
                .put(handlers::crud::put_rolebinding)
                .delete(handlers::crud::delete_rolebinding),
        );

    let protected_routes = match state.config.mode {
        ApiMode::ReadOnly => flux_routes,
        ApiMode::Crud => flux_routes.merge(crud_routes),
    }
    .layer(middleware::from_fn_with_state(
        state.clone(),
        auth::bearer_auth,
    ));

    Router::new()
        .route("/health", get(handlers::health::health))
        .route("/ready", get(handlers::health::health))
        .route("/openapi.yaml", get(get_openapi))
        .merge(protected_routes)
        .with_state(state)
}

async fn get_openapi(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/yaml")],
        state.openapi_doc.as_str().to_string(),
    )
}
