use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ComponentCatalogDoc {
    #[serde(rename = "_id")]
    pub id: String,
    pub component_path: String,
    pub component_version: String,
    #[serde(default)]
    pub cluster_env_enabled: bool,
    pub oci_url: String,
    pub oci_tag: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
}
