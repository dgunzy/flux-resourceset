use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};

use crate::AppState;
use crate::domain::{
    ClusterComponentRef, ClusterDoc, ClusterNamespaceRef, ClusterRolebindingRef,
    ComponentCatalogDoc, NamespaceDoc, RolebindingDoc,
};
use crate::error::AppError;

#[derive(Debug, serde::Deserialize)]
pub struct ClusterListQuery {
    pub cluster_dns: Option<String>,
    pub environment: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ComponentListQuery {
    pub component_version: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, serde::Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

pub async fn list_clusters(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ClusterListQuery>,
) -> Result<Json<Vec<crate::models::Cluster>>, AppError> {
    let rows = state
        .store
        .list_clusters(
            query.cluster_dns.as_deref(),
            query.environment.as_deref(),
            query.limit,
            query.offset,
        )
        .await?;

    Ok(Json(rows.iter().map(domain_cluster_to_api).collect()))
}

pub async fn create_cluster(
    State(state): State<Arc<AppState>>,
    Json(body): Json<crate::models::CreateCluster>,
) -> Result<Json<crate::models::Cluster>, AppError> {
    let cluster = create_cluster_to_domain(body)?;
    validate_cluster_references(&state, &cluster).await?;
    let created = state.store.create_cluster(cluster).await?;
    Ok(Json(domain_cluster_to_api(&created)))
}

pub async fn get_cluster(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<crate::models::Cluster>, AppError> {
    let cluster = state
        .store
        .get_cluster(&id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(domain_cluster_to_api(&cluster)))
}

pub async fn put_cluster(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(update): Json<crate::models::UpdateCluster>,
) -> Result<Json<crate::models::Cluster>, AppError> {
    let mut cluster = state
        .store
        .get_cluster(&id)
        .await?
        .ok_or(AppError::NotFound)?;
    apply_cluster_update(&mut cluster, update);
    validate_cluster_references(&state, &cluster).await?;
    let updated = state.store.put_cluster(&id, cluster).await?;
    Ok(Json(domain_cluster_to_api(&updated)))
}

pub async fn delete_cluster(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<crate::models::Cluster>, AppError> {
    let deleted = state.store.delete_cluster(&id).await?;
    Ok(Json(domain_cluster_to_api(&deleted)))
}

pub async fn list_platform_components(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ComponentListQuery>,
) -> Result<Json<Vec<crate::models::PlatformComponent>>, AppError> {
    let rows = state
        .store
        .list_platform_components(
            query.component_version.as_deref(),
            query.limit,
            query.offset,
        )
        .await?;

    Ok(Json(rows.iter().map(domain_component_to_api).collect()))
}

pub async fn create_platform_component(
    State(state): State<Arc<AppState>>,
    Json(body): Json<crate::models::CreatePlatformComponent>,
) -> Result<Json<crate::models::PlatformComponent>, AppError> {
    let component = create_component_to_domain(body)?;
    let created = state.store.create_platform_component(component).await?;
    Ok(Json(domain_component_to_api(&created)))
}

pub async fn get_platform_component(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<crate::models::PlatformComponent>, AppError> {
    let component = state
        .store
        .get_platform_component(&id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(domain_component_to_api(&component)))
}

pub async fn put_platform_component(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(update): Json<crate::models::UpdatePlatformComponent>,
) -> Result<Json<crate::models::PlatformComponent>, AppError> {
    let mut component = state
        .store
        .get_platform_component(&id)
        .await?
        .ok_or(AppError::NotFound)?;

    apply_component_update(&mut component, update);
    let updated = state.store.put_platform_component(&id, component).await?;
    Ok(Json(domain_component_to_api(&updated)))
}

pub async fn delete_platform_component(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<crate::models::PlatformComponent>, AppError> {
    let deleted = state.store.delete_platform_component(&id).await?;
    Ok(Json(domain_component_to_api(&deleted)))
}

pub async fn list_namespaces(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<Vec<crate::models::Namespace>>, AppError> {
    let rows = state
        .store
        .list_namespaces(query.limit, query.offset)
        .await?;
    Ok(Json(rows.iter().map(domain_namespace_to_api).collect()))
}

pub async fn create_namespace(
    State(state): State<Arc<AppState>>,
    Json(body): Json<crate::models::CreateNamespace>,
) -> Result<Json<crate::models::Namespace>, AppError> {
    let namespace = create_namespace_to_domain(body)?;
    let created = state.store.create_namespace(namespace).await?;
    Ok(Json(domain_namespace_to_api(&created)))
}

pub async fn get_namespace(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<crate::models::Namespace>, AppError> {
    let namespace = state
        .store
        .get_namespace(&id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(domain_namespace_to_api(&namespace)))
}

pub async fn put_namespace(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(update): Json<crate::models::UpdateNamespace>,
) -> Result<Json<crate::models::Namespace>, AppError> {
    let mut namespace = state
        .store
        .get_namespace(&id)
        .await?
        .ok_or(AppError::NotFound)?;

    if let Some(labels) = update.labels {
        namespace.labels = labels;
    }
    if let Some(annotations) = update.annotations {
        namespace.annotations = annotations;
    }

    let updated = state.store.put_namespace(&id, namespace).await?;
    Ok(Json(domain_namespace_to_api(&updated)))
}

pub async fn delete_namespace(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<crate::models::Namespace>, AppError> {
    let deleted = state.store.delete_namespace(&id).await?;
    Ok(Json(domain_namespace_to_api(&deleted)))
}

pub async fn list_rolebindings(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<Vec<crate::models::Rolebinding>>, AppError> {
    let rows = state
        .store
        .list_rolebindings(query.limit, query.offset)
        .await?;
    Ok(Json(rows.iter().map(domain_rolebinding_to_api).collect()))
}

pub async fn create_rolebinding(
    State(state): State<Arc<AppState>>,
    Json(body): Json<crate::models::CreateRolebinding>,
) -> Result<Json<crate::models::Rolebinding>, AppError> {
    let rolebinding = create_rolebinding_to_domain(body)?;
    let created = state.store.create_rolebinding(rolebinding).await?;
    Ok(Json(domain_rolebinding_to_api(&created)))
}

pub async fn get_rolebinding(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<crate::models::Rolebinding>, AppError> {
    let rolebinding = state
        .store
        .get_rolebinding(&id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(domain_rolebinding_to_api(&rolebinding)))
}

pub async fn put_rolebinding(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(update): Json<crate::models::UpdateRolebinding>,
) -> Result<Json<crate::models::Rolebinding>, AppError> {
    let mut rolebinding = state
        .store
        .get_rolebinding(&id)
        .await?
        .ok_or(AppError::NotFound)?;

    if let Some(role) = update.role {
        rolebinding.role = role;
    }
    if let Some(subjects) = update.subjects {
        rolebinding.subjects = subjects_to_values(subjects);
    }

    let updated = state.store.put_rolebinding(&id, rolebinding).await?;
    Ok(Json(domain_rolebinding_to_api(&updated)))
}

pub async fn delete_rolebinding(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<crate::models::Rolebinding>, AppError> {
    let deleted = state.store.delete_rolebinding(&id).await?;
    Ok(Json(domain_rolebinding_to_api(&deleted)))
}

fn required_string_id(id: Option<serde_json::Value>, field: &str) -> Result<String, AppError> {
    let value = id.ok_or_else(|| AppError::Validation(format!("{field} is required")))?;
    match value {
        serde_json::Value::String(s) if !s.trim().is_empty() => Ok(s),
        _ => Err(AppError::Validation(format!(
            "{field} must be a non-empty string"
        ))),
    }
}

fn id_json(id: &str) -> Option<Option<serde_json::Value>> {
    Some(Some(serde_json::Value::String(id.to_string())))
}

fn create_cluster_to_domain(input: crate::models::CreateCluster) -> Result<ClusterDoc, AppError> {
    Ok(ClusterDoc {
        id: required_string_id(input.id, "id")?,
        cluster_name: input.cluster_name,
        cluster_dns: input.cluster_dns,
        environment: match input.environment {
            crate::models::create_cluster::Environment::Dev => "dev",
            crate::models::create_cluster::Environment::Qa => "qa",
            crate::models::create_cluster::Environment::Uat => "uat",
            crate::models::create_cluster::Environment::Prod => "prod",
        }
        .to_string(),
        node_count: input.node_count,
        vm_image: input.vm_image,
        k0s_version: input.k0s_version,
        platform_components: input
            .platform_components
            .into_iter()
            .map(|pc| ClusterComponentRef {
                id: pc.id,
                enabled: pc.enabled,
                oci_tag: pc.oci_tag,
                component_path: pc.component_path,
            })
            .collect(),
        namespaces: input
            .namespaces
            .unwrap_or_default()
            .into_iter()
            .map(|ns| ClusterNamespaceRef { id: ns.id })
            .collect(),
        rolebindings: input
            .rolebindings
            .unwrap_or_default()
            .into_iter()
            .map(|rb| ClusterRolebindingRef { id: rb.id })
            .collect(),
        patches: input.patches.unwrap_or_default(),
    })
}

fn apply_cluster_update(cluster: &mut ClusterDoc, update: crate::models::UpdateCluster) {
    if let Some(cluster_name) = update.cluster_name {
        cluster.cluster_name = cluster_name;
    }
    if let Some(cluster_dns) = update.cluster_dns {
        cluster.cluster_dns = cluster_dns;
    }
    if let Some(environment) = update.environment {
        cluster.environment = match environment {
            crate::models::update_cluster::Environment::Dev => "dev",
            crate::models::update_cluster::Environment::Qa => "qa",
            crate::models::update_cluster::Environment::Uat => "uat",
            crate::models::update_cluster::Environment::Prod => "prod",
        }
        .to_string();
    }
    if let Some(node_count) = update.node_count {
        cluster.node_count = Some(node_count);
    }
    if let Some(vm_image) = update.vm_image {
        cluster.vm_image = Some(vm_image);
    }
    if let Some(k0s_version) = update.k0s_version {
        cluster.k0s_version = Some(k0s_version);
    }
    if let Some(platform_components) = update.platform_components {
        cluster.platform_components = platform_components
            .into_iter()
            .map(|pc| ClusterComponentRef {
                id: pc.id,
                enabled: pc.enabled,
                oci_tag: pc.oci_tag,
                component_path: pc.component_path,
            })
            .collect();
    }
    if let Some(namespaces) = update.namespaces {
        cluster.namespaces = namespaces
            .into_iter()
            .map(|ns| ClusterNamespaceRef { id: ns.id })
            .collect();
    }
    if let Some(rolebindings) = update.rolebindings {
        cluster.rolebindings = rolebindings
            .into_iter()
            .map(|rb| ClusterRolebindingRef { id: rb.id })
            .collect();
    }
    if let Some(patches) = update.patches {
        cluster.patches = patches;
    }
}

fn domain_cluster_to_api(cluster: &ClusterDoc) -> crate::models::Cluster {
    crate::models::Cluster {
        id: id_json(&cluster.id),
        cluster_name: Some(cluster.cluster_name.clone()),
        cluster_dns: Some(cluster.cluster_dns.clone()),
        environment: Some(match cluster.environment.as_str() {
            "qa" => crate::models::cluster::Environment::Qa,
            "uat" => crate::models::cluster::Environment::Uat,
            "prod" => crate::models::cluster::Environment::Prod,
            _ => crate::models::cluster::Environment::Dev,
        }),
        node_count: cluster.node_count,
        vm_image: cluster.vm_image.clone(),
        k0s_version: cluster.k0s_version.clone(),
        platform_components: Some(
            cluster
                .platform_components
                .iter()
                .map(|pc| crate::models::ClusterPlatformComponentsInner {
                    id: pc.id.clone(),
                    enabled: pc.enabled,
                    oci_tag: pc.oci_tag.clone(),
                    component_path: pc.component_path.clone(),
                })
                .collect(),
        ),
        namespaces: Some(
            cluster
                .namespaces
                .iter()
                .map(|ns| crate::models::ClusterNamespacesInner { id: ns.id.clone() })
                .collect(),
        ),
        rolebindings: Some(
            cluster
                .rolebindings
                .iter()
                .map(|rb| crate::models::ClusterNamespacesInner { id: rb.id.clone() })
                .collect(),
        ),
        patches: Some(cluster.patches.clone()),
    }
}

fn create_component_to_domain(
    input: crate::models::CreatePlatformComponent,
) -> Result<ComponentCatalogDoc, AppError> {
    Ok(ComponentCatalogDoc {
        id: required_string_id(input.id, "id")?,
        component_path: input.component_path,
        component_version: input.component_version,
        cluster_env_enabled: input.cluster_env_enabled.unwrap_or(false),
        oci_url: input.oci_url,
        oci_tag: input.oci_tag,
        depends_on: input.depends_on,
    })
}

fn apply_component_update(
    component: &mut ComponentCatalogDoc,
    update: crate::models::UpdatePlatformComponent,
) {
    if let Some(component_path) = update.component_path {
        component.component_path = component_path;
    }
    if let Some(component_version) = update.component_version {
        component.component_version = component_version;
    }
    if let Some(cluster_env_enabled) = update.cluster_env_enabled {
        component.cluster_env_enabled = cluster_env_enabled;
    }
    if let Some(oci_url) = update.oci_url {
        component.oci_url = oci_url;
    }
    if let Some(oci_tag) = update.oci_tag {
        component.oci_tag = oci_tag;
    }
    if let Some(depends_on) = update.depends_on {
        component.depends_on = depends_on;
    }
}

fn domain_component_to_api(component: &ComponentCatalogDoc) -> crate::models::PlatformComponent {
    crate::models::PlatformComponent {
        id: id_json(&component.id),
        component_path: Some(component.component_path.clone()),
        component_version: Some(component.component_version.clone()),
        cluster_env_enabled: Some(component.cluster_env_enabled),
        oci_url: Some(component.oci_url.clone()),
        oci_tag: Some(component.oci_tag.clone()),
        depends_on: Some(component.depends_on.clone()),
    }
}

fn create_namespace_to_domain(
    input: crate::models::CreateNamespace,
) -> Result<NamespaceDoc, AppError> {
    Ok(NamespaceDoc {
        id: required_string_id(input.id, "id")?,
        labels: input.labels.unwrap_or_default(),
        annotations: input.annotations.unwrap_or_default(),
    })
}

fn domain_namespace_to_api(namespace: &NamespaceDoc) -> crate::models::Namespace {
    crate::models::Namespace {
        id: id_json(&namespace.id),
        labels: Some(namespace.labels.clone()),
        annotations: Some(namespace.annotations.clone()),
    }
}

fn create_rolebinding_to_domain(
    input: crate::models::CreateRolebinding,
) -> Result<RolebindingDoc, AppError> {
    Ok(RolebindingDoc {
        id: required_string_id(input.id, "id")?,
        role: input.role,
        subjects: subjects_to_values(input.subjects),
    })
}

fn domain_rolebinding_to_api(rolebinding: &RolebindingDoc) -> crate::models::Rolebinding {
    crate::models::Rolebinding {
        id: id_json(&rolebinding.id),
        role: Some(rolebinding.role.clone()),
        subjects: Some(values_to_subject_maps(&rolebinding.subjects)),
    }
}

fn subjects_to_values(subjects: Vec<HashMap<String, serde_json::Value>>) -> Vec<serde_json::Value> {
    subjects
        .into_iter()
        .map(|subject| {
            let object = subject
                .into_iter()
                .collect::<serde_json::Map<String, serde_json::Value>>();
            serde_json::Value::Object(object)
        })
        .collect()
}

fn values_to_subject_maps(values: &[serde_json::Value]) -> Vec<HashMap<String, serde_json::Value>> {
    values
        .iter()
        .filter_map(|value| {
            let serde_json::Value::Object(map) = value else {
                return None;
            };
            Some(
                map.iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect::<HashMap<String, serde_json::Value>>(),
            )
        })
        .collect()
}

async fn validate_cluster_references(
    state: &Arc<AppState>,
    cluster: &ClusterDoc,
) -> Result<(), AppError> {
    ensure_unique_ids(
        cluster.platform_components.iter().map(|pc| pc.id.as_str()),
        "platform component",
    )?;
    ensure_unique_ids(
        cluster.namespaces.iter().map(|ns| ns.id.as_str()),
        "namespace",
    )?;
    ensure_unique_ids(
        cluster.rolebindings.iter().map(|rb| rb.id.as_str()),
        "rolebinding",
    )?;

    for component in &cluster.platform_components {
        if state
            .store
            .get_platform_component(&component.id)
            .await?
            .is_none()
        {
            return Err(AppError::Validation(format!(
                "cluster references unknown platform component '{}'",
                component.id
            )));
        }
    }

    for namespace in &cluster.namespaces {
        if state.store.get_namespace(&namespace.id).await?.is_none() {
            return Err(AppError::Validation(format!(
                "cluster references unknown namespace '{}'",
                namespace.id
            )));
        }
    }

    for rolebinding in &cluster.rolebindings {
        if state
            .store
            .get_rolebinding(&rolebinding.id)
            .await?
            .is_none()
        {
            return Err(AppError::Validation(format!(
                "cluster references unknown rolebinding '{}'",
                rolebinding.id
            )));
        }
    }

    for component_id in cluster.patches.keys() {
        if !cluster
            .platform_components
            .iter()
            .any(|component| &component.id == component_id)
        {
            return Err(AppError::Validation(format!(
                "patches references unknown cluster platform component '{}'",
                component_id
            )));
        }
    }

    Ok(())
}

fn ensure_unique_ids<'a>(
    ids: impl Iterator<Item = &'a str>,
    resource_name: &str,
) -> Result<(), AppError> {
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(AppError::Validation(format!(
                "duplicate {resource_name} id '{id}' in cluster payload"
            )));
        }
    }
    Ok(())
}
