use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use std::sync::Arc;
use tower::ServiceExt;

use flux_resourceset::AppState;
use flux_resourceset::build_router;
use flux_resourceset::config::{ApiMode, Config, StoreBackend};
use flux_resourceset::db::Store;

type TestApp = (axum::Router, tempfile::NamedTempFile);

fn crud_test_app() -> TestApp {
    let tmp = tempfile::NamedTempFile::new().expect("create temp file");
    std::fs::copy("data/seed.json", tmp.path()).expect("copy seed data");

    let config = Config {
        mode: ApiMode::Crud,
        store_backend: StoreBackend::InMemory,
        database_url: "sqlite::memory:".into(),
        auth_token: "read-token".into(),
        crud_auth_token: Some("write-token".into()),
        seed_file: tmp.path().to_string_lossy().to_string(),
        openapi_file: "openapi/openapi.yaml".into(),
        listen_addr: "127.0.0.1:0".into(),
    };

    let store = Store::in_memory_from_file(&config.seed_file).expect("load data store");
    let openapi_doc = std::fs::read_to_string(&config.openapi_file).expect("load openapi");
    let state = Arc::new(AppState {
        store,
        config,
        openapi_doc: Arc::new(openapi_doc),
    });

    (build_router(state), tmp)
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
async fn test_crud_namespace_assignment_reflected_in_flux_endpoint() {
    let (app, _tmp) = crud_test_app();

    let create_namespace = serde_json::json!({
      "id": "demo-dynamic",
      "labels": {"team": "demo"},
      "annotations": {"owner": "cli"}
    });

    let resp = app
        .clone()
        .oneshot(
            Request::post("/namespaces")
                .header("authorization", "Bearer write-token")
                .header("content-type", "application/json")
                .body(Body::from(create_namespace.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let update_cluster = serde_json::json!({
      "namespaces": [
        {
          "id": "demo-dynamic",
          "labels": {"team": "demo"},
          "annotations": {"owner": "cli"}
        }
      ]
    });

    let resp = app
        .clone()
        .oneshot(
            Request::put("/clusters/demo-cluster-01")
                .header("authorization", "Bearer write-token")
                .header("content-type", "application/json")
                .body(Body::from(update_cluster.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = app
        .oneshot(
            Request::get("/api/v2/flux/clusters/demo-cluster-01.k8s.example.com/namespaces")
                .header("authorization", "Bearer read-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = serde_json::from_slice(&get_body(resp).await).unwrap();
    let inputs = body["inputs"].as_array().unwrap();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0]["id"], "demo-dynamic");
    assert_eq!(inputs[0]["labels"]["team"], "demo");
    assert_eq!(inputs[0]["annotations"]["owner"], "cli");
}

#[tokio::test]
async fn test_crud_write_token_required_for_post() {
    let (app, _tmp) = crud_test_app();

    let create_namespace = serde_json::json!({"id": "nope"});

    let resp = app
        .oneshot(
            Request::post("/namespaces")
                .header("authorization", "Bearer read-token")
                .header("content-type", "application/json")
                .body(Body::from(create_namespace.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_cluster_update_rejects_unknown_namespace_reference() {
    let (app, _tmp) = crud_test_app();

    let update_cluster = serde_json::json!({
      "namespaces": [
        {
          "id": "namespace-does-not-exist",
          "labels": {"team": "demo"},
          "annotations": {}
        }
      ]
    });

    let resp = app
        .oneshot(
            Request::put("/clusters/demo-cluster-01")
                .header("authorization", "Bearer write-token")
                .header("content-type", "application/json")
                .body(Body::from(update_cluster.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: serde_json::Value = serde_json::from_slice(&get_body(resp).await).unwrap();
    assert_eq!(body["error"], "validation_error");
    assert!(
        body["message"]
            .as_str()
            .unwrap_or_default()
            .contains("unknown namespace")
    );
}

#[tokio::test]
async fn test_cluster_update_rejects_duplicate_component_ids() {
    let (app, _tmp) = crud_test_app();

    let update_cluster = serde_json::json!({
      "platform_components": [
        {"id": "podinfo", "enabled": true, "oci_tag": null, "component_path": null},
        {"id": "podinfo", "enabled": false, "oci_tag": null, "component_path": null}
      ]
    });

    let resp = app
        .oneshot(
            Request::put("/clusters/demo-cluster-01")
                .header("authorization", "Bearer write-token")
                .header("content-type", "application/json")
                .body(Body::from(update_cluster.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: serde_json::Value = serde_json::from_slice(&get_body(resp).await).unwrap();
    assert_eq!(body["error"], "validation_error");
    assert!(
        body["message"]
            .as_str()
            .unwrap_or_default()
            .contains("duplicate platform component id")
    );
}

#[tokio::test]
async fn test_generic_component_patch_keys_reflected_in_flux_payload() {
    let (app, _tmp) = crud_test_app();

    let patch_update = serde_json::json!({
      "patches": {
        "podinfo": {
          "replicaCount": "3",
          "ui.message": "patched-through-crud-api"
        }
      }
    });

    let resp = app
        .clone()
        .oneshot(
            Request::put("/clusters/demo-cluster-01")
                .header("authorization", "Bearer write-token")
                .header("content-type", "application/json")
                .body(Body::from(patch_update.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = app
        .oneshot(
            Request::get(
                "/api/v2/flux/clusters/demo-cluster-01.k8s.example.com/platform-components",
            )
            .header("authorization", "Bearer read-token")
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = serde_json::from_slice(&get_body(resp).await).unwrap();
    let inputs = body["inputs"].as_array().unwrap();
    let podinfo = inputs.iter().find(|i| i["id"] == "podinfo").unwrap();
    assert_eq!(podinfo["patches"]["replicaCount"], "3");
    assert_eq!(podinfo["patches"]["ui.message"], "patched-through-crud-api");
}
