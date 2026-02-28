use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};

use crate::AppState;
use crate::error::AppError;
use crate::merge::{self, FluxResponse, NamespaceInput};

pub async fn get_namespaces(
    State(state): State<Arc<AppState>>,
    Path(cluster_dns): Path<String>,
) -> Result<Json<FluxResponse<NamespaceInput>>, AppError> {
    let cluster = state
        .store
        .get_cluster_by_dns(&cluster_dns)
        .await?
        .ok_or(AppError::NotFound)?;

    let namespace_ids: Vec<&str> = cluster.namespaces.iter().map(|ns| ns.id.as_str()).collect();
    let namespaces = state.store.get_namespaces_by_ids(&namespace_ids).await?;

    Ok(Json(merge::merge_namespaces(&cluster, &namespaces)))
}
