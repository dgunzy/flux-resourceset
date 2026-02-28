use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClusterDoc {
    #[serde(rename = "_id")]
    pub id: String,
    pub cluster_name: String,
    pub cluster_dns: String,
    pub environment: String,
    #[serde(default)]
    pub node_count: Option<i32>,
    #[serde(default)]
    pub vm_image: Option<String>,
    #[serde(default)]
    pub k0s_version: Option<String>,
    pub platform_components: Vec<ClusterComponentRef>,
    #[serde(default)]
    pub namespaces: Vec<ClusterNamespaceRef>,
    #[serde(default)]
    pub rolebindings: Vec<ClusterRolebindingRef>,
    #[serde(default)]
    pub patches: HashMap<String, HashMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClusterComponentRef {
    pub id: String,
    pub enabled: bool,
    pub oci_tag: Option<String>,
    pub component_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClusterNamespaceRef {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClusterRolebindingRef {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NamespaceDoc {
    pub id: String,
    #[serde(default)]
    pub labels: HashMap<String, String>,
    #[serde(default)]
    pub annotations: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RolebindingDoc {
    pub id: String,
    pub role: String,
    pub subjects: Vec<serde_json::Value>,
}
