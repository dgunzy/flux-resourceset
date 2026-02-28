use std::sync::Arc;

use flux_resourceset::config::{ApiMode, Config, StoreBackend};
use flux_resourceset::db::Store;
use flux_resourceset::{AppState, build_router};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = Config::from_env();
    let store = match config.store_backend {
        StoreBackend::InMemory => Store::in_memory_from_file(&config.seed_file)
            .unwrap_or_else(|e| panic!("Failed to load seed file {}: {e}", config.seed_file)),
        StoreBackend::Sqlite => {
            let database_url = config.database_url.clone();
            let seed_file = config.seed_file.clone();
            Store::sqlite_from_seed(&database_url, &seed_file)
                .await
                .unwrap_or_else(|e| {
                    panic!(
                        "Failed to initialize sqlite store {} with seed {}: {e}",
                        database_url, seed_file
                    )
                })
        }
    };

    let openapi_doc = std::fs::read_to_string(&config.openapi_file)
        .unwrap_or_else(|e| panic!("Failed to load OpenAPI file {}: {e}", config.openapi_file));

    let listen_addr = config.listen_addr.clone();
    let mode = config.mode;
    let state = Arc::new(AppState {
        store: Arc::new(store),
        config,
        openapi_doc: Arc::new(openapi_doc),
    });

    let app = build_router(state).layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&listen_addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind to {listen_addr}: {e}"));

    let mode_label = match mode {
        ApiMode::ReadOnly => "read-only",
        ApiMode::Crud => "crud",
    };

    tracing::info!(mode = mode_label, addr = %listen_addr, "Listening");
    axum::serve(listener, app).await.unwrap();
}
