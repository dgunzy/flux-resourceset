use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Row, SqlitePool};

use crate::db::DataStoreError;
use crate::domain::{ClusterDoc, ComponentCatalogDoc, NamespaceDoc, RolebindingDoc};

#[derive(Debug, Clone)]
pub struct SqliteStore {
    pool: SqlitePool,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct SeedData {
    clusters: Vec<ClusterDoc>,
    platform_components: Vec<ComponentCatalogDoc>,
    #[serde(default)]
    namespaces: Vec<NamespaceDoc>,
    #[serde(default)]
    rolebindings: Vec<RolebindingDoc>,
}

impl SqliteStore {
    pub async fn connect(
        database_url: &str,
        seed_file: impl AsRef<Path>,
    ) -> Result<Self, DataStoreError> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .after_connect(|conn, _meta| {
                Box::pin(async move {
                    sqlx::query("PRAGMA busy_timeout = 5000;")
                        .execute(&mut *conn)
                        .await?;
                    Ok(())
                })
            })
            .connect(database_url)
            .await?;
        let store = Self { pool };
        store.init_schema().await?;
        store.seed_if_empty(seed_file.as_ref()).await?;
        Ok(store)
    }

    async fn init_schema(&self) -> Result<(), DataStoreError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS clusters (
                id TEXT PRIMARY KEY,
                data TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS components (
                id TEXT PRIMARY KEY,
                data TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS namespaces (
                id TEXT PRIMARY KEY,
                data TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS rolebindings (
                id TEXT PRIMARY KEY,
                data TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn seed_if_empty(&self, seed_file: &Path) -> Result<(), DataStoreError> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(1) FROM clusters")
            .fetch_one(&self.pool)
            .await?;
        if count > 0 {
            return Ok(());
        }

        let content = std::fs::read_to_string(seed_file)?;
        let seed: SeedData = serde_json::from_str(&content)?;
        let mut tx = self.pool.begin().await?;

        for cluster in seed.clusters {
            let payload = serde_json::to_string(&cluster)?;
            sqlx::query("INSERT OR IGNORE INTO clusters (id, data) VALUES (?1, ?2)")
                .bind(cluster.id)
                .bind(payload)
                .execute(&mut *tx)
                .await?;
        }

        for component in seed.platform_components {
            let payload = serde_json::to_string(&component)?;
            sqlx::query("INSERT OR IGNORE INTO components (id, data) VALUES (?1, ?2)")
                .bind(component.id)
                .bind(payload)
                .execute(&mut *tx)
                .await?;
        }

        for namespace in seed.namespaces {
            let payload = serde_json::to_string(&namespace)?;
            sqlx::query("INSERT OR IGNORE INTO namespaces (id, data) VALUES (?1, ?2)")
                .bind(namespace.id)
                .bind(payload)
                .execute(&mut *tx)
                .await?;
        }

        for rolebinding in seed.rolebindings {
            let payload = serde_json::to_string(&rolebinding)?;
            sqlx::query("INSERT OR IGNORE INTO rolebindings (id, data) VALUES (?1, ?2)")
                .bind(rolebinding.id)
                .bind(payload)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn fetch_doc<T: serde::de::DeserializeOwned>(
        &self,
        table: &str,
        id: &str,
    ) -> Result<Option<T>, DataStoreError> {
        let query = format!("SELECT data FROM {table} WHERE id = ?1");
        let row = sqlx::query(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(row) => {
                let payload: String = row.try_get("data")?;
                Ok(Some(serde_json::from_str(&payload)?))
            }
            None => Ok(None),
        }
    }

    async fn fetch_all_docs<T: serde::de::DeserializeOwned>(
        &self,
        table: &str,
    ) -> Result<Vec<T>, DataStoreError> {
        let query = format!("SELECT data FROM {table}");
        let rows = sqlx::query(&query).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                let payload: String = row.try_get("data")?;
                Ok::<_, DataStoreError>(serde_json::from_str(&payload)?)
            })
            .collect()
    }

    async fn upsert_doc<T: Serialize>(
        &self,
        table: &str,
        id: &str,
        value: &T,
    ) -> Result<(), DataStoreError> {
        let payload = serde_json::to_string(value)?;
        let query = format!(
            "INSERT INTO {table} (id, data) VALUES (?1, ?2) \
             ON CONFLICT(id) DO UPDATE SET data = excluded.data"
        );
        sqlx::query(&query)
            .bind(id)
            .bind(payload)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn delete_doc<T: serde::de::DeserializeOwned>(
        &self,
        table: &str,
        resource_name: &str,
        id: &str,
    ) -> Result<T, DataStoreError> {
        let value = self
            .fetch_doc(table, id)
            .await?
            .ok_or_else(|| DataStoreError::NotFound(format!("{resource_name} '{id}'")))?;
        let query = format!("DELETE FROM {table} WHERE id = ?1");
        sqlx::query(&query).bind(id).execute(&self.pool).await?;
        Ok(value)
    }

    pub async fn get_cluster_by_dns(
        &self,
        dns: &str,
    ) -> Result<Option<ClusterDoc>, DataStoreError> {
        let all = self.get_all_clusters().await?;
        Ok(all.into_iter().find(|cluster| cluster.cluster_dns == dns))
    }

    pub async fn get_all_clusters(&self) -> Result<Vec<ClusterDoc>, DataStoreError> {
        let mut rows = self.fetch_all_docs::<ClusterDoc>("clusters").await?;
        rows.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(rows)
    }

    pub async fn get_components_by_ids(
        &self,
        ids: &[&str],
    ) -> Result<HashMap<String, ComponentCatalogDoc>, DataStoreError> {
        let all = self
            .fetch_all_docs::<ComponentCatalogDoc>("components")
            .await?;
        let all_map: HashMap<_, _> = all.into_iter().map(|c| (c.id.clone(), c)).collect();
        Ok(ids
            .iter()
            .filter_map(|id| all_map.get(*id).map(|c| (c.id.clone(), c.clone())))
            .collect())
    }

    pub async fn get_namespaces_by_ids(
        &self,
        ids: &[&str],
    ) -> Result<HashMap<String, NamespaceDoc>, DataStoreError> {
        let all = self.fetch_all_docs::<NamespaceDoc>("namespaces").await?;
        let all_map: HashMap<_, _> = all.into_iter().map(|n| (n.id.clone(), n)).collect();
        Ok(ids
            .iter()
            .filter_map(|id| all_map.get(*id).map(|n| (n.id.clone(), n.clone())))
            .collect())
    }

    pub async fn get_rolebindings_by_ids(
        &self,
        ids: &[&str],
    ) -> Result<HashMap<String, RolebindingDoc>, DataStoreError> {
        let all = self
            .fetch_all_docs::<RolebindingDoc>("rolebindings")
            .await?;
        let all_map: HashMap<_, _> = all.into_iter().map(|rb| (rb.id.clone(), rb)).collect();
        Ok(ids
            .iter()
            .filter_map(|id| all_map.get(*id).map(|rb| (rb.id.clone(), rb.clone())))
            .collect())
    }

    pub async fn list_clusters(
        &self,
        cluster_dns: Option<&str>,
        environment: Option<&str>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<ClusterDoc>, DataStoreError> {
        let mut rows = self
            .get_all_clusters()
            .await?
            .into_iter()
            .filter(|cluster| match cluster_dns {
                Some(dns) => cluster.cluster_dns == dns,
                None => true,
            })
            .filter(|cluster| match environment {
                Some(env) => cluster.environment == env,
                None => true,
            })
            .collect::<Vec<_>>();
        rows.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(apply_pagination(rows, limit, offset))
    }

    pub async fn get_cluster(&self, id: &str) -> Result<Option<ClusterDoc>, DataStoreError> {
        self.fetch_doc("clusters", id).await
    }

    pub async fn create_cluster(&self, cluster: ClusterDoc) -> Result<ClusterDoc, DataStoreError> {
        if self.get_cluster(&cluster.id).await?.is_some() {
            return Err(DataStoreError::Conflict(format!(
                "cluster '{}'",
                cluster.id
            )));
        }
        self.upsert_doc("clusters", &cluster.id, &cluster).await?;
        Ok(cluster)
    }

    pub async fn put_cluster(
        &self,
        id: &str,
        mut cluster: ClusterDoc,
    ) -> Result<ClusterDoc, DataStoreError> {
        cluster.id = id.to_string();
        self.upsert_doc("clusters", id, &cluster).await?;
        Ok(cluster)
    }

    pub async fn delete_cluster(&self, id: &str) -> Result<ClusterDoc, DataStoreError> {
        self.delete_doc("clusters", "cluster", id).await
    }

    pub async fn list_platform_components(
        &self,
        component_version: Option<&str>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<ComponentCatalogDoc>, DataStoreError> {
        let mut rows = self
            .fetch_all_docs::<ComponentCatalogDoc>("components")
            .await?
            .into_iter()
            .filter(|component| match component_version {
                Some(version) => component.component_version == version,
                None => true,
            })
            .collect::<Vec<_>>();
        rows.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(apply_pagination(rows, limit, offset))
    }

    pub async fn get_platform_component(
        &self,
        id: &str,
    ) -> Result<Option<ComponentCatalogDoc>, DataStoreError> {
        self.fetch_doc("components", id).await
    }

    pub async fn create_platform_component(
        &self,
        component: ComponentCatalogDoc,
    ) -> Result<ComponentCatalogDoc, DataStoreError> {
        if self.get_platform_component(&component.id).await?.is_some() {
            return Err(DataStoreError::Conflict(format!(
                "platform component '{}'",
                component.id
            )));
        }
        self.upsert_doc("components", &component.id, &component)
            .await?;
        Ok(component)
    }

    pub async fn put_platform_component(
        &self,
        id: &str,
        mut component: ComponentCatalogDoc,
    ) -> Result<ComponentCatalogDoc, DataStoreError> {
        component.id = id.to_string();
        self.upsert_doc("components", id, &component).await?;
        Ok(component)
    }

    pub async fn delete_platform_component(
        &self,
        id: &str,
    ) -> Result<ComponentCatalogDoc, DataStoreError> {
        self.delete_doc("components", "platform component", id)
            .await
    }

    pub async fn list_namespaces(
        &self,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<NamespaceDoc>, DataStoreError> {
        let mut rows = self.fetch_all_docs::<NamespaceDoc>("namespaces").await?;
        rows.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(apply_pagination(rows, limit, offset))
    }

    pub async fn get_namespace(&self, id: &str) -> Result<Option<NamespaceDoc>, DataStoreError> {
        self.fetch_doc("namespaces", id).await
    }

    pub async fn create_namespace(
        &self,
        namespace: NamespaceDoc,
    ) -> Result<NamespaceDoc, DataStoreError> {
        if self.get_namespace(&namespace.id).await?.is_some() {
            return Err(DataStoreError::Conflict(format!(
                "namespace '{}'",
                namespace.id
            )));
        }
        self.upsert_doc("namespaces", &namespace.id, &namespace)
            .await?;
        Ok(namespace)
    }

    pub async fn put_namespace(
        &self,
        id: &str,
        mut namespace: NamespaceDoc,
    ) -> Result<NamespaceDoc, DataStoreError> {
        namespace.id = id.to_string();
        self.upsert_doc("namespaces", id, &namespace).await?;
        Ok(namespace)
    }

    pub async fn delete_namespace(&self, id: &str) -> Result<NamespaceDoc, DataStoreError> {
        self.delete_doc("namespaces", "namespace", id).await
    }

    pub async fn list_rolebindings(
        &self,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<RolebindingDoc>, DataStoreError> {
        let mut rows = self
            .fetch_all_docs::<RolebindingDoc>("rolebindings")
            .await?;
        rows.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(apply_pagination(rows, limit, offset))
    }

    pub async fn get_rolebinding(
        &self,
        id: &str,
    ) -> Result<Option<RolebindingDoc>, DataStoreError> {
        self.fetch_doc("rolebindings", id).await
    }

    pub async fn create_rolebinding(
        &self,
        rolebinding: RolebindingDoc,
    ) -> Result<RolebindingDoc, DataStoreError> {
        if self.get_rolebinding(&rolebinding.id).await?.is_some() {
            return Err(DataStoreError::Conflict(format!(
                "rolebinding '{}'",
                rolebinding.id
            )));
        }
        self.upsert_doc("rolebindings", &rolebinding.id, &rolebinding)
            .await?;
        Ok(rolebinding)
    }

    pub async fn put_rolebinding(
        &self,
        id: &str,
        mut rolebinding: RolebindingDoc,
    ) -> Result<RolebindingDoc, DataStoreError> {
        rolebinding.id = id.to_string();
        self.upsert_doc("rolebindings", id, &rolebinding).await?;
        Ok(rolebinding)
    }

    pub async fn delete_rolebinding(&self, id: &str) -> Result<RolebindingDoc, DataStoreError> {
        self.delete_doc("rolebindings", "rolebinding", id).await
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
