use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::domain::{ClusterDoc, ComponentCatalogDoc, NamespaceDoc, RolebindingDoc};
use crate::sqlite_store::SqliteStore;

#[derive(Debug, thiserror::Error)]
pub enum DataStoreError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("resource conflict: {0}")]
    Conflict(String),
    #[error("resource not found: {0}")]
    NotFound(String),
}

#[derive(Debug, Clone)]
pub struct InMemoryStore {
    inner: Arc<RwLock<StoreData>>,
    data_file: Option<PathBuf>,
}

#[derive(Debug, Default)]
struct StoreData {
    clusters: HashMap<String, ClusterDoc>,
    components: HashMap<String, ComponentCatalogDoc>,
    namespaces: HashMap<String, NamespaceDoc>,
    rolebindings: HashMap<String, RolebindingDoc>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct SeedData {
    clusters: Vec<ClusterDoc>,
    platform_components: Vec<ComponentCatalogDoc>,
    #[serde(default)]
    namespaces: Vec<NamespaceDoc>,
    #[serde(default)]
    rolebindings: Vec<RolebindingDoc>,
}

impl InMemoryStore {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, DataStoreError> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)?;
        Self::from_json_with_file(&content, Some(path.to_path_buf()))
    }

    pub fn from_json(json: &str) -> Result<Self, DataStoreError> {
        Self::from_json_with_file(json, None)
    }

    fn from_json_with_file(json: &str, data_file: Option<PathBuf>) -> Result<Self, DataStoreError> {
        let seed: SeedData = serde_json::from_str(json)?;
        Ok(Self::from_seed_data(seed, data_file))
    }

    fn from_seed_data(seed: SeedData, data_file: Option<PathBuf>) -> Self {
        let mut data = StoreData::default();

        for cluster in seed.clusters {
            data.clusters.insert(cluster.id.clone(), cluster);
        }

        for component in seed.platform_components {
            data.components.insert(component.id.clone(), component);
        }

        for namespace in seed.namespaces {
            data.namespaces.insert(namespace.id.clone(), namespace);
        }

        for rolebinding in seed.rolebindings {
            data.rolebindings
                .insert(rolebinding.id.clone(), rolebinding);
        }

        Self {
            inner: Arc::new(RwLock::new(data)),
            data_file,
        }
    }

    fn persist(&self, data: &StoreData) -> Result<(), DataStoreError> {
        let Some(path) = &self.data_file else {
            return Ok(());
        };

        let mut clusters: Vec<_> = data.clusters.values().cloned().collect();
        let mut platform_components: Vec<_> = data.components.values().cloned().collect();
        let mut namespaces: Vec<_> = data.namespaces.values().cloned().collect();
        let mut rolebindings: Vec<_> = data.rolebindings.values().cloned().collect();

        clusters.sort_by(|a, b| a.id.cmp(&b.id));
        platform_components.sort_by(|a, b| a.id.cmp(&b.id));
        namespaces.sort_by(|a, b| a.id.cmp(&b.id));
        rolebindings.sort_by(|a, b| a.id.cmp(&b.id));

        let seed = SeedData {
            clusters,
            platform_components,
            namespaces,
            rolebindings,
        };

        let tmp_path = path.with_extension("json.tmp");
        let serialized = serde_json::to_string_pretty(&seed)?;
        std::fs::write(&tmp_path, serialized)?;
        std::fs::rename(tmp_path, path)?;
        Ok(())
    }

    pub async fn get_cluster_by_dns(
        &self,
        dns: &str,
    ) -> Result<Option<ClusterDoc>, DataStoreError> {
        let data = self.inner.read().await;
        Ok(data
            .clusters
            .values()
            .find(|cluster| cluster.cluster_dns == dns)
            .cloned())
    }

    pub async fn get_all_clusters(&self) -> Result<Vec<ClusterDoc>, DataStoreError> {
        let data = self.inner.read().await;
        let mut clusters: Vec<_> = data.clusters.values().cloned().collect();
        clusters.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(clusters)
    }

    pub async fn get_components_by_ids(
        &self,
        ids: &[&str],
    ) -> Result<HashMap<String, ComponentCatalogDoc>, DataStoreError> {
        let data = self.inner.read().await;
        Ok(ids
            .iter()
            .filter_map(|id| {
                data.components
                    .get(*id)
                    .map(|component| (component.id.clone(), component.clone()))
            })
            .collect())
    }

    pub async fn get_namespaces_by_ids(
        &self,
        ids: &[&str],
    ) -> Result<HashMap<String, NamespaceDoc>, DataStoreError> {
        let data = self.inner.read().await;
        Ok(ids
            .iter()
            .filter_map(|id| {
                data.namespaces
                    .get(*id)
                    .map(|namespace| (namespace.id.clone(), namespace.clone()))
            })
            .collect())
    }

    pub async fn get_rolebindings_by_ids(
        &self,
        ids: &[&str],
    ) -> Result<HashMap<String, RolebindingDoc>, DataStoreError> {
        let data = self.inner.read().await;
        Ok(ids
            .iter()
            .filter_map(|id| {
                data.rolebindings
                    .get(*id)
                    .map(|rolebinding| (rolebinding.id.clone(), rolebinding.clone()))
            })
            .collect())
    }

    pub async fn list_clusters(
        &self,
        cluster_dns: Option<&str>,
        environment: Option<&str>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<ClusterDoc>, DataStoreError> {
        let data = self.inner.read().await;
        let mut rows: Vec<_> = data
            .clusters
            .values()
            .filter(|cluster| match cluster_dns {
                Some(dns) => cluster.cluster_dns == dns,
                None => true,
            })
            .filter(|cluster| match environment {
                Some(env) => cluster.environment == env,
                None => true,
            })
            .cloned()
            .collect();
        rows.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(apply_pagination(rows, limit, offset))
    }

    pub async fn get_cluster(&self, id: &str) -> Result<Option<ClusterDoc>, DataStoreError> {
        let data = self.inner.read().await;
        Ok(data.clusters.get(id).cloned())
    }

    pub async fn create_cluster(&self, cluster: ClusterDoc) -> Result<ClusterDoc, DataStoreError> {
        let mut data = self.inner.write().await;
        if data.clusters.contains_key(&cluster.id) {
            return Err(DataStoreError::Conflict(format!(
                "cluster '{}'",
                cluster.id
            )));
        }
        data.clusters.insert(cluster.id.clone(), cluster.clone());
        self.persist(&data)?;
        Ok(cluster)
    }

    pub async fn put_cluster(
        &self,
        id: &str,
        mut cluster: ClusterDoc,
    ) -> Result<ClusterDoc, DataStoreError> {
        cluster.id = id.to_string();
        let mut data = self.inner.write().await;
        data.clusters.insert(id.to_string(), cluster.clone());
        self.persist(&data)?;
        Ok(cluster)
    }

    pub async fn delete_cluster(&self, id: &str) -> Result<ClusterDoc, DataStoreError> {
        let mut data = self.inner.write().await;
        let removed = data
            .clusters
            .remove(id)
            .ok_or_else(|| DataStoreError::NotFound(format!("cluster '{}'", id)))?;
        self.persist(&data)?;
        Ok(removed)
    }

    pub async fn list_platform_components(
        &self,
        component_version: Option<&str>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<ComponentCatalogDoc>, DataStoreError> {
        let data = self.inner.read().await;
        let mut rows: Vec<_> = data
            .components
            .values()
            .filter(|component| match component_version {
                Some(version) => component.component_version == version,
                None => true,
            })
            .cloned()
            .collect();
        rows.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(apply_pagination(rows, limit, offset))
    }

    pub async fn get_platform_component(
        &self,
        id: &str,
    ) -> Result<Option<ComponentCatalogDoc>, DataStoreError> {
        let data = self.inner.read().await;
        Ok(data.components.get(id).cloned())
    }

    pub async fn create_platform_component(
        &self,
        component: ComponentCatalogDoc,
    ) -> Result<ComponentCatalogDoc, DataStoreError> {
        let mut data = self.inner.write().await;
        if data.components.contains_key(&component.id) {
            return Err(DataStoreError::Conflict(format!(
                "platform component '{}'",
                component.id
            )));
        }
        data.components
            .insert(component.id.clone(), component.clone());
        self.persist(&data)?;
        Ok(component)
    }

    pub async fn put_platform_component(
        &self,
        id: &str,
        mut component: ComponentCatalogDoc,
    ) -> Result<ComponentCatalogDoc, DataStoreError> {
        component.id = id.to_string();
        let mut data = self.inner.write().await;
        data.components.insert(id.to_string(), component.clone());
        self.persist(&data)?;
        Ok(component)
    }

    pub async fn delete_platform_component(
        &self,
        id: &str,
    ) -> Result<ComponentCatalogDoc, DataStoreError> {
        let mut data = self.inner.write().await;
        let removed = data
            .components
            .remove(id)
            .ok_or_else(|| DataStoreError::NotFound(format!("platform component '{}'", id)))?;
        self.persist(&data)?;
        Ok(removed)
    }

    pub async fn list_namespaces(
        &self,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<NamespaceDoc>, DataStoreError> {
        let data = self.inner.read().await;
        let mut rows: Vec<_> = data.namespaces.values().cloned().collect();
        rows.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(apply_pagination(rows, limit, offset))
    }

    pub async fn get_namespace(&self, id: &str) -> Result<Option<NamespaceDoc>, DataStoreError> {
        let data = self.inner.read().await;
        Ok(data.namespaces.get(id).cloned())
    }

    pub async fn create_namespace(
        &self,
        namespace: NamespaceDoc,
    ) -> Result<NamespaceDoc, DataStoreError> {
        let mut data = self.inner.write().await;
        if data.namespaces.contains_key(&namespace.id) {
            return Err(DataStoreError::Conflict(format!(
                "namespace '{}'",
                namespace.id
            )));
        }
        data.namespaces
            .insert(namespace.id.clone(), namespace.clone());
        self.persist(&data)?;
        Ok(namespace)
    }

    pub async fn put_namespace(
        &self,
        id: &str,
        mut namespace: NamespaceDoc,
    ) -> Result<NamespaceDoc, DataStoreError> {
        namespace.id = id.to_string();
        let mut data = self.inner.write().await;
        data.namespaces.insert(id.to_string(), namespace.clone());
        self.persist(&data)?;
        Ok(namespace)
    }

    pub async fn delete_namespace(&self, id: &str) -> Result<NamespaceDoc, DataStoreError> {
        let mut data = self.inner.write().await;
        let removed = data
            .namespaces
            .remove(id)
            .ok_or_else(|| DataStoreError::NotFound(format!("namespace '{}'", id)))?;
        self.persist(&data)?;
        Ok(removed)
    }

    pub async fn list_rolebindings(
        &self,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<RolebindingDoc>, DataStoreError> {
        let data = self.inner.read().await;
        let mut rows: Vec<_> = data.rolebindings.values().cloned().collect();
        rows.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(apply_pagination(rows, limit, offset))
    }

    pub async fn get_rolebinding(
        &self,
        id: &str,
    ) -> Result<Option<RolebindingDoc>, DataStoreError> {
        let data = self.inner.read().await;
        Ok(data.rolebindings.get(id).cloned())
    }

    pub async fn create_rolebinding(
        &self,
        rolebinding: RolebindingDoc,
    ) -> Result<RolebindingDoc, DataStoreError> {
        let mut data = self.inner.write().await;
        if data.rolebindings.contains_key(&rolebinding.id) {
            return Err(DataStoreError::Conflict(format!(
                "rolebinding '{}'",
                rolebinding.id
            )));
        }
        data.rolebindings
            .insert(rolebinding.id.clone(), rolebinding.clone());
        self.persist(&data)?;
        Ok(rolebinding)
    }

    pub async fn put_rolebinding(
        &self,
        id: &str,
        mut rolebinding: RolebindingDoc,
    ) -> Result<RolebindingDoc, DataStoreError> {
        rolebinding.id = id.to_string();
        let mut data = self.inner.write().await;
        data.rolebindings
            .insert(id.to_string(), rolebinding.clone());
        self.persist(&data)?;
        Ok(rolebinding)
    }

    pub async fn delete_rolebinding(&self, id: &str) -> Result<RolebindingDoc, DataStoreError> {
        let mut data = self.inner.write().await;
        let removed = data
            .rolebindings
            .remove(id)
            .ok_or_else(|| DataStoreError::NotFound(format!("rolebinding '{}'", id)))?;
        self.persist(&data)?;
        Ok(removed)
    }
}

#[derive(Debug, Clone)]
pub enum Store {
    InMemory(InMemoryStore),
    Sqlite(SqliteStore),
}

impl Store {
    pub fn in_memory_from_file(path: impl AsRef<Path>) -> Result<Self, DataStoreError> {
        Ok(Self::InMemory(InMemoryStore::load(path)?))
    }

    pub fn in_memory_from_json(json: &str) -> Result<Self, DataStoreError> {
        Ok(Self::InMemory(InMemoryStore::from_json(json)?))
    }

    pub async fn sqlite_from_seed(
        database_url: &str,
        seed_file: impl AsRef<Path>,
    ) -> Result<Self, DataStoreError> {
        Ok(Self::Sqlite(
            SqliteStore::connect(database_url, seed_file).await?,
        ))
    }

    pub async fn get_cluster_by_dns(
        &self,
        dns: &str,
    ) -> Result<Option<ClusterDoc>, DataStoreError> {
        match self {
            Self::InMemory(store) => store.get_cluster_by_dns(dns).await,
            Self::Sqlite(store) => store.get_cluster_by_dns(dns).await,
        }
    }

    pub async fn get_all_clusters(&self) -> Result<Vec<ClusterDoc>, DataStoreError> {
        match self {
            Self::InMemory(store) => store.get_all_clusters().await,
            Self::Sqlite(store) => store.get_all_clusters().await,
        }
    }

    pub async fn get_components_by_ids(
        &self,
        ids: &[&str],
    ) -> Result<HashMap<String, ComponentCatalogDoc>, DataStoreError> {
        match self {
            Self::InMemory(store) => store.get_components_by_ids(ids).await,
            Self::Sqlite(store) => store.get_components_by_ids(ids).await,
        }
    }

    pub async fn get_namespaces_by_ids(
        &self,
        ids: &[&str],
    ) -> Result<HashMap<String, NamespaceDoc>, DataStoreError> {
        match self {
            Self::InMemory(store) => store.get_namespaces_by_ids(ids).await,
            Self::Sqlite(store) => store.get_namespaces_by_ids(ids).await,
        }
    }

    pub async fn get_rolebindings_by_ids(
        &self,
        ids: &[&str],
    ) -> Result<HashMap<String, RolebindingDoc>, DataStoreError> {
        match self {
            Self::InMemory(store) => store.get_rolebindings_by_ids(ids).await,
            Self::Sqlite(store) => store.get_rolebindings_by_ids(ids).await,
        }
    }

    pub async fn list_clusters(
        &self,
        cluster_dns: Option<&str>,
        environment: Option<&str>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<ClusterDoc>, DataStoreError> {
        match self {
            Self::InMemory(store) => {
                store
                    .list_clusters(cluster_dns, environment, limit, offset)
                    .await
            }
            Self::Sqlite(store) => {
                store
                    .list_clusters(cluster_dns, environment, limit, offset)
                    .await
            }
        }
    }

    pub async fn get_cluster(&self, id: &str) -> Result<Option<ClusterDoc>, DataStoreError> {
        match self {
            Self::InMemory(store) => store.get_cluster(id).await,
            Self::Sqlite(store) => store.get_cluster(id).await,
        }
    }

    pub async fn create_cluster(&self, cluster: ClusterDoc) -> Result<ClusterDoc, DataStoreError> {
        match self {
            Self::InMemory(store) => store.create_cluster(cluster).await,
            Self::Sqlite(store) => store.create_cluster(cluster).await,
        }
    }

    pub async fn put_cluster(
        &self,
        id: &str,
        cluster: ClusterDoc,
    ) -> Result<ClusterDoc, DataStoreError> {
        match self {
            Self::InMemory(store) => store.put_cluster(id, cluster).await,
            Self::Sqlite(store) => store.put_cluster(id, cluster).await,
        }
    }

    pub async fn delete_cluster(&self, id: &str) -> Result<ClusterDoc, DataStoreError> {
        match self {
            Self::InMemory(store) => store.delete_cluster(id).await,
            Self::Sqlite(store) => store.delete_cluster(id).await,
        }
    }

    pub async fn list_platform_components(
        &self,
        component_version: Option<&str>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<ComponentCatalogDoc>, DataStoreError> {
        match self {
            Self::InMemory(store) => {
                store
                    .list_platform_components(component_version, limit, offset)
                    .await
            }
            Self::Sqlite(store) => {
                store
                    .list_platform_components(component_version, limit, offset)
                    .await
            }
        }
    }

    pub async fn get_platform_component(
        &self,
        id: &str,
    ) -> Result<Option<ComponentCatalogDoc>, DataStoreError> {
        match self {
            Self::InMemory(store) => store.get_platform_component(id).await,
            Self::Sqlite(store) => store.get_platform_component(id).await,
        }
    }

    pub async fn create_platform_component(
        &self,
        component: ComponentCatalogDoc,
    ) -> Result<ComponentCatalogDoc, DataStoreError> {
        match self {
            Self::InMemory(store) => store.create_platform_component(component).await,
            Self::Sqlite(store) => store.create_platform_component(component).await,
        }
    }

    pub async fn put_platform_component(
        &self,
        id: &str,
        component: ComponentCatalogDoc,
    ) -> Result<ComponentCatalogDoc, DataStoreError> {
        match self {
            Self::InMemory(store) => store.put_platform_component(id, component).await,
            Self::Sqlite(store) => store.put_platform_component(id, component).await,
        }
    }

    pub async fn delete_platform_component(
        &self,
        id: &str,
    ) -> Result<ComponentCatalogDoc, DataStoreError> {
        match self {
            Self::InMemory(store) => store.delete_platform_component(id).await,
            Self::Sqlite(store) => store.delete_platform_component(id).await,
        }
    }

    pub async fn list_namespaces(
        &self,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<NamespaceDoc>, DataStoreError> {
        match self {
            Self::InMemory(store) => store.list_namespaces(limit, offset).await,
            Self::Sqlite(store) => store.list_namespaces(limit, offset).await,
        }
    }

    pub async fn get_namespace(&self, id: &str) -> Result<Option<NamespaceDoc>, DataStoreError> {
        match self {
            Self::InMemory(store) => store.get_namespace(id).await,
            Self::Sqlite(store) => store.get_namespace(id).await,
        }
    }

    pub async fn create_namespace(
        &self,
        namespace: NamespaceDoc,
    ) -> Result<NamespaceDoc, DataStoreError> {
        match self {
            Self::InMemory(store) => store.create_namespace(namespace).await,
            Self::Sqlite(store) => store.create_namespace(namespace).await,
        }
    }

    pub async fn put_namespace(
        &self,
        id: &str,
        namespace: NamespaceDoc,
    ) -> Result<NamespaceDoc, DataStoreError> {
        match self {
            Self::InMemory(store) => store.put_namespace(id, namespace).await,
            Self::Sqlite(store) => store.put_namespace(id, namespace).await,
        }
    }

    pub async fn delete_namespace(&self, id: &str) -> Result<NamespaceDoc, DataStoreError> {
        match self {
            Self::InMemory(store) => store.delete_namespace(id).await,
            Self::Sqlite(store) => store.delete_namespace(id).await,
        }
    }

    pub async fn list_rolebindings(
        &self,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<RolebindingDoc>, DataStoreError> {
        match self {
            Self::InMemory(store) => store.list_rolebindings(limit, offset).await,
            Self::Sqlite(store) => store.list_rolebindings(limit, offset).await,
        }
    }

    pub async fn get_rolebinding(
        &self,
        id: &str,
    ) -> Result<Option<RolebindingDoc>, DataStoreError> {
        match self {
            Self::InMemory(store) => store.get_rolebinding(id).await,
            Self::Sqlite(store) => store.get_rolebinding(id).await,
        }
    }

    pub async fn create_rolebinding(
        &self,
        rolebinding: RolebindingDoc,
    ) -> Result<RolebindingDoc, DataStoreError> {
        match self {
            Self::InMemory(store) => store.create_rolebinding(rolebinding).await,
            Self::Sqlite(store) => store.create_rolebinding(rolebinding).await,
        }
    }

    pub async fn put_rolebinding(
        &self,
        id: &str,
        rolebinding: RolebindingDoc,
    ) -> Result<RolebindingDoc, DataStoreError> {
        match self {
            Self::InMemory(store) => store.put_rolebinding(id, rolebinding).await,
            Self::Sqlite(store) => store.put_rolebinding(id, rolebinding).await,
        }
    }

    pub async fn delete_rolebinding(&self, id: &str) -> Result<RolebindingDoc, DataStoreError> {
        match self {
            Self::InMemory(store) => store.delete_rolebinding(id).await,
            Self::Sqlite(store) => store.delete_rolebinding(id).await,
        }
    }
}

fn apply_pagination<T: Clone>(rows: Vec<T>, limit: Option<usize>, offset: Option<usize>) -> Vec<T> {
    let offset = offset.unwrap_or(0);
    let iter = rows.into_iter().skip(offset);
    match limit {
        Some(limit) => iter.take(limit).collect(),
        None => iter.collect(),
    }
}

#[async_trait]
pub trait DataStore: Send + Sync {
    async fn get_cluster_by_dns(&self, dns: &str) -> Result<Option<ClusterDoc>, DataStoreError>;
    async fn get_all_clusters(&self) -> Result<Vec<ClusterDoc>, DataStoreError>;
    async fn get_components_by_ids(
        &self,
        ids: &[&str],
    ) -> Result<HashMap<String, ComponentCatalogDoc>, DataStoreError>;
    async fn get_namespaces_by_ids(
        &self,
        ids: &[&str],
    ) -> Result<HashMap<String, NamespaceDoc>, DataStoreError>;
    async fn get_rolebindings_by_ids(
        &self,
        ids: &[&str],
    ) -> Result<HashMap<String, RolebindingDoc>, DataStoreError>;
    async fn list_clusters(
        &self,
        cluster_dns: Option<&str>,
        environment: Option<&str>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<ClusterDoc>, DataStoreError>;
    async fn get_cluster(&self, id: &str) -> Result<Option<ClusterDoc>, DataStoreError>;
    async fn create_cluster(&self, cluster: ClusterDoc) -> Result<ClusterDoc, DataStoreError>;
    async fn put_cluster(
        &self,
        id: &str,
        cluster: ClusterDoc,
    ) -> Result<ClusterDoc, DataStoreError>;
    async fn delete_cluster(&self, id: &str) -> Result<ClusterDoc, DataStoreError>;
    async fn list_platform_components(
        &self,
        component_version: Option<&str>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<ComponentCatalogDoc>, DataStoreError>;
    async fn get_platform_component(
        &self,
        id: &str,
    ) -> Result<Option<ComponentCatalogDoc>, DataStoreError>;
    async fn create_platform_component(
        &self,
        component: ComponentCatalogDoc,
    ) -> Result<ComponentCatalogDoc, DataStoreError>;
    async fn put_platform_component(
        &self,
        id: &str,
        component: ComponentCatalogDoc,
    ) -> Result<ComponentCatalogDoc, DataStoreError>;
    async fn delete_platform_component(
        &self,
        id: &str,
    ) -> Result<ComponentCatalogDoc, DataStoreError>;
    async fn list_namespaces(
        &self,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<NamespaceDoc>, DataStoreError>;
    async fn get_namespace(&self, id: &str) -> Result<Option<NamespaceDoc>, DataStoreError>;
    async fn create_namespace(
        &self,
        namespace: NamespaceDoc,
    ) -> Result<NamespaceDoc, DataStoreError>;
    async fn put_namespace(
        &self,
        id: &str,
        namespace: NamespaceDoc,
    ) -> Result<NamespaceDoc, DataStoreError>;
    async fn delete_namespace(&self, id: &str) -> Result<NamespaceDoc, DataStoreError>;
    async fn list_rolebindings(
        &self,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<RolebindingDoc>, DataStoreError>;
    async fn get_rolebinding(&self, id: &str) -> Result<Option<RolebindingDoc>, DataStoreError>;
    async fn create_rolebinding(
        &self,
        rolebinding: RolebindingDoc,
    ) -> Result<RolebindingDoc, DataStoreError>;
    async fn put_rolebinding(
        &self,
        id: &str,
        rolebinding: RolebindingDoc,
    ) -> Result<RolebindingDoc, DataStoreError>;
    async fn delete_rolebinding(&self, id: &str) -> Result<RolebindingDoc, DataStoreError>;
}

#[async_trait]
impl DataStore for Store {
    async fn get_cluster_by_dns(&self, dns: &str) -> Result<Option<ClusterDoc>, DataStoreError> {
        Store::get_cluster_by_dns(self, dns).await
    }

    async fn get_all_clusters(&self) -> Result<Vec<ClusterDoc>, DataStoreError> {
        Store::get_all_clusters(self).await
    }

    async fn get_components_by_ids(
        &self,
        ids: &[&str],
    ) -> Result<HashMap<String, ComponentCatalogDoc>, DataStoreError> {
        Store::get_components_by_ids(self, ids).await
    }

    async fn get_namespaces_by_ids(
        &self,
        ids: &[&str],
    ) -> Result<HashMap<String, NamespaceDoc>, DataStoreError> {
        Store::get_namespaces_by_ids(self, ids).await
    }

    async fn get_rolebindings_by_ids(
        &self,
        ids: &[&str],
    ) -> Result<HashMap<String, RolebindingDoc>, DataStoreError> {
        Store::get_rolebindings_by_ids(self, ids).await
    }

    async fn list_clusters(
        &self,
        cluster_dns: Option<&str>,
        environment: Option<&str>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<ClusterDoc>, DataStoreError> {
        Store::list_clusters(self, cluster_dns, environment, limit, offset).await
    }

    async fn get_cluster(&self, id: &str) -> Result<Option<ClusterDoc>, DataStoreError> {
        Store::get_cluster(self, id).await
    }

    async fn create_cluster(&self, cluster: ClusterDoc) -> Result<ClusterDoc, DataStoreError> {
        Store::create_cluster(self, cluster).await
    }

    async fn put_cluster(
        &self,
        id: &str,
        cluster: ClusterDoc,
    ) -> Result<ClusterDoc, DataStoreError> {
        Store::put_cluster(self, id, cluster).await
    }

    async fn delete_cluster(&self, id: &str) -> Result<ClusterDoc, DataStoreError> {
        Store::delete_cluster(self, id).await
    }

    async fn list_platform_components(
        &self,
        component_version: Option<&str>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<ComponentCatalogDoc>, DataStoreError> {
        Store::list_platform_components(self, component_version, limit, offset).await
    }

    async fn get_platform_component(
        &self,
        id: &str,
    ) -> Result<Option<ComponentCatalogDoc>, DataStoreError> {
        Store::get_platform_component(self, id).await
    }

    async fn create_platform_component(
        &self,
        component: ComponentCatalogDoc,
    ) -> Result<ComponentCatalogDoc, DataStoreError> {
        Store::create_platform_component(self, component).await
    }

    async fn put_platform_component(
        &self,
        id: &str,
        component: ComponentCatalogDoc,
    ) -> Result<ComponentCatalogDoc, DataStoreError> {
        Store::put_platform_component(self, id, component).await
    }

    async fn delete_platform_component(
        &self,
        id: &str,
    ) -> Result<ComponentCatalogDoc, DataStoreError> {
        Store::delete_platform_component(self, id).await
    }

    async fn list_namespaces(
        &self,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<NamespaceDoc>, DataStoreError> {
        Store::list_namespaces(self, limit, offset).await
    }

    async fn get_namespace(&self, id: &str) -> Result<Option<NamespaceDoc>, DataStoreError> {
        Store::get_namespace(self, id).await
    }

    async fn create_namespace(
        &self,
        namespace: NamespaceDoc,
    ) -> Result<NamespaceDoc, DataStoreError> {
        Store::create_namespace(self, namespace).await
    }

    async fn put_namespace(
        &self,
        id: &str,
        namespace: NamespaceDoc,
    ) -> Result<NamespaceDoc, DataStoreError> {
        Store::put_namespace(self, id, namespace).await
    }

    async fn delete_namespace(&self, id: &str) -> Result<NamespaceDoc, DataStoreError> {
        Store::delete_namespace(self, id).await
    }

    async fn list_rolebindings(
        &self,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<RolebindingDoc>, DataStoreError> {
        Store::list_rolebindings(self, limit, offset).await
    }

    async fn get_rolebinding(&self, id: &str) -> Result<Option<RolebindingDoc>, DataStoreError> {
        Store::get_rolebinding(self, id).await
    }

    async fn create_rolebinding(
        &self,
        rolebinding: RolebindingDoc,
    ) -> Result<RolebindingDoc, DataStoreError> {
        Store::create_rolebinding(self, rolebinding).await
    }

    async fn put_rolebinding(
        &self,
        id: &str,
        rolebinding: RolebindingDoc,
    ) -> Result<RolebindingDoc, DataStoreError> {
        Store::put_rolebinding(self, id, rolebinding).await
    }

    async fn delete_rolebinding(&self, id: &str) -> Result<RolebindingDoc, DataStoreError> {
        Store::delete_rolebinding(self, id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_JSON: &str = r#"{
        "clusters": [{
            "_id": "test-01",
            "cluster_name": "test-01",
            "cluster_dns": "test-01.example.com",
            "environment": "dev",
            "platform_components": [
                { "id": "podinfo", "enabled": true, "oci_tag": null, "component_path": null }
            ],
            "namespaces": [{ "id": "default", "labels": {}, "annotations": {} }],
            "rolebindings": [],
            "patches": {}
        }],
        "platform_components": [{
            "_id": "podinfo",
            "component_path": "apps/podinfo/6.5.0",
            "component_version": "6.5.0",
            "cluster_env_enabled": false,
            "oci_url": "oci://ghcr.io/stefanprodan/manifests/podinfo",
            "oci_tag": "v1.0.0",
            "depends_on": []
        }],
        "namespaces": [{ "id": "default", "labels": {}, "annotations": {} }],
        "rolebindings": []
    }"#;

    #[tokio::test]
    async fn test_load_and_query() {
        let store = InMemoryStore::from_json(TEST_JSON).unwrap();

        let cluster = store
            .get_cluster_by_dns("test-01.example.com")
            .await
            .unwrap();
        assert!(cluster.is_some());
        let cluster = cluster.unwrap();
        assert_eq!(cluster.cluster_name, "test-01");

        let missing = store.get_cluster_by_dns("nonexistent").await.unwrap();
        assert!(missing.is_none());

        let all = store.get_all_clusters().await.unwrap();
        assert_eq!(all.len(), 1);

        let comps = store
            .get_components_by_ids(&["podinfo", "missing"])
            .await
            .unwrap();
        assert_eq!(comps.len(), 1);
        assert!(comps.contains_key("podinfo"));

        let namespaces = store.list_namespaces(None, None).await.unwrap();
        assert_eq!(namespaces.len(), 1);
    }
}
