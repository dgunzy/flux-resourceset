use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};

use crate::AppState;
use crate::error::AppError;
use crate::merge::{self, FluxResponse, RolebindingInput};

pub async fn get_rolebindings(
    State(state): State<Arc<AppState>>,
    Path(cluster_dns): Path<String>,
) -> Result<Json<FluxResponse<RolebindingInput>>, AppError> {
    let cluster = state
        .store
        .get_cluster_by_dns(&cluster_dns)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Json(merge::merge_rolebindings(&cluster)))
}
