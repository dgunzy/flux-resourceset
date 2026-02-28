use std::sync::Arc;

use axum::Json;
use axum::extract::State;

use crate::AppState;
use crate::error::AppError;
use crate::merge::{self, ClusterListInput, FluxResponse};

pub async fn get_clusters(
    State(state): State<Arc<AppState>>,
) -> Result<Json<FluxResponse<ClusterListInput>>, AppError> {
    let clusters = state.store.get_all_clusters().await?;
    Ok(Json(merge::merge_clusters(&clusters)))
}
