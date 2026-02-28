use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};

use crate::AppState;
use crate::error::AppError;
use crate::merge::{self, FluxResponse, PlatformComponentInput};

pub async fn get_platform_components(
    State(state): State<Arc<AppState>>,
    Path(cluster_dns): Path<String>,
) -> Result<Json<FluxResponse<PlatformComponentInput>>, AppError> {
    let cluster = state
        .store
        .get_cluster_by_dns(&cluster_dns)
        .await?
        .ok_or(AppError::NotFound)?;

    let component_ids: Vec<&str> = cluster
        .platform_components
        .iter()
        .map(|c| c.id.as_str())
        .collect();

    let catalog = state.store.get_components_by_ids(&component_ids).await?;

    Ok(Json(merge::merge_platform_components(&cluster, &catalog)))
}
