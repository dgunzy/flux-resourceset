use axum::extract::Request;
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::Response;

use crate::config::ApiMode;

pub async fn bearer_auth(
    state: axum::extract::State<std::sync::Arc<crate::AppState>>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let header = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let expected = match (state.config.mode, req.method()) {
        (_, &Method::GET | &Method::HEAD | &Method::OPTIONS) => state.config.auth_token.as_str(),
        (ApiMode::Crud, _) => state
            .config
            .crud_auth_token
            .as_deref()
            .unwrap_or(state.config.auth_token.as_str()),
        (ApiMode::ReadOnly, _) => return Err(StatusCode::METHOD_NOT_ALLOWED),
    };

    let expected_header = format!("Bearer {expected}");
    if header == expected_header {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}
