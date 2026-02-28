use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use std::sync::Arc;
use tower::ServiceExt;

use flux_resourceset::AppState;
use flux_resourceset::build_router;
use flux_resourceset::config::{ApiMode, Config, StoreBackend};
use flux_resourceset::db::Store;

fn test_config() -> Config {
    Config {
        mode: ApiMode::ReadOnly,
        store_backend: StoreBackend::InMemory,
        database_url: "sqlite::memory:".into(),
        auth_token: "test-token".into(),
        crud_auth_token: None,
        seed_file: "data/seed.json".into(),
        openapi_file: "openapi/openapi.yaml".into(),
        listen_addr: "127.0.0.1:0".into(),
    }
}

fn test_app() -> axum::Router {
    let config = test_config();
    let store = Store::in_memory_from_file(&config.seed_file).unwrap();
    let openapi_doc = std::fs::read_to_string(&config.openapi_file).unwrap();
    let state = Arc::new(AppState {
        store,
        config,
        openapi_doc: Arc::new(openapi_doc),
    });

    build_router(state)
}

async fn get_body(resp: axum::response::Response) -> Vec<u8> {
    resp.into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec()
}

#[tokio::test]
async fn test_health() {
    let app = test_app();
    let resp = app
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(&get_body(resp).await).unwrap();
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn test_missing_auth_returns_401() {
    let app = test_app();
    let resp = app
        .oneshot(
            Request::get("/api/v2/flux/clusters")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_wrong_auth_returns_401() {
    let app = test_app();
    let resp = app
        .oneshot(
            Request::get("/api/v2/flux/clusters")
                .header("authorization", "Bearer wrong-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_clusters_with_auth() {
    let app = test_app();
    let resp = app
        .oneshot(
            Request::get("/api/v2/flux/clusters")
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(&get_body(resp).await).unwrap();
    assert!(body["inputs"].is_array());
    assert!(!body["inputs"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_platform_components() {
    let app = test_app();
    let resp = app
        .oneshot(
            Request::get(
                "/api/v2/flux/clusters/demo-cluster-01.k8s.example.com/platform-components",
            )
            .header("authorization", "Bearer test-token")
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(&get_body(resp).await).unwrap();
    let inputs = body["inputs"].as_array().unwrap();
    assert_eq!(inputs.len(), 3);

    let podinfo = inputs.iter().find(|i| i["id"] == "podinfo").unwrap();
    assert_eq!(podinfo["cluster"]["name"], "demo-cluster-01");
    assert_eq!(
        podinfo["source"]["oci_url"],
        "https://stefanprodan.github.io/podinfo"
    );
    assert_eq!(podinfo["source"]["oci_tag"], "latest");
    assert_eq!(podinfo["cluster_env_enabled"], false);
    assert_eq!(podinfo["patches"]["replicaCount"], "2");
    assert_eq!(
        podinfo["patches"]["ui.message"],
        "Hello from Flux ResourceSet dynamic patches"
    );

    let cm = inputs.iter().find(|i| i["id"] == "cert-manager").unwrap();
    assert_eq!(cm["source"]["oci_url"], "https://charts.jetstack.io");
    assert_eq!(cm["source"]["oci_tag"], "latest");

    let traefik = inputs.iter().find(|i| i["id"] == "traefik").unwrap();
    assert_eq!(traefik["component_path"], "traefik");
    assert_eq!(traefik["patches"]["service.type"], "ClusterIP");
    assert_eq!(traefik["depends_on"][0], "cert-manager");
}

#[tokio::test]
async fn test_namespaces() {
    let app = test_app();
    let resp = app
        .oneshot(
            Request::get("/api/v2/flux/clusters/demo-cluster-01.k8s.example.com/namespaces")
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(&get_body(resp).await).unwrap();
    let inputs = body["inputs"].as_array().unwrap();
    assert_eq!(inputs.len(), 3);
}

#[tokio::test]
async fn test_rolebindings() {
    let app = test_app();
    let resp = app
        .oneshot(
            Request::get("/api/v2/flux/clusters/demo-cluster-01.k8s.example.com/rolebindings")
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(&get_body(resp).await).unwrap();
    let inputs = body["inputs"].as_array().unwrap();
    assert_eq!(inputs.len(), 2);
    assert_eq!(inputs[0]["role"], "cluster-admin");
}

#[tokio::test]
async fn test_unknown_cluster_returns_404() {
    let app = test_app();
    let resp = app
        .oneshot(
            Request::get("/api/v2/flux/clusters/nonexistent.example.com/platform-components")
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_contract_unique_ids() {
    let app = test_app();
    let resp = app
        .oneshot(
            Request::get(
                "/api/v2/flux/clusters/demo-cluster-01.k8s.example.com/platform-components",
            )
            .header("authorization", "Bearer test-token")
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&get_body(resp).await).unwrap();
    let inputs = body["inputs"].as_array().unwrap();

    let ids: Vec<&str> = inputs.iter().map(|i| i["id"].as_str().unwrap()).collect();
    let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
    assert_eq!(ids.len(), unique.len(), "IDs must be unique");
}

#[tokio::test]
async fn test_contract_response_size() {
    let app = test_app();
    let resp = app
        .oneshot(
            Request::get(
                "/api/v2/flux/clusters/demo-cluster-01.k8s.example.com/platform-components",
            )
            .header("authorization", "Bearer test-token")
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    let body = get_body(resp).await;
    assert!(body.len() < 900 * 1024, "Response must be under 900 KiB");
}
