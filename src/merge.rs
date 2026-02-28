use std::collections::HashMap;

use serde::Serialize;

use crate::domain::{ClusterDoc, ComponentCatalogDoc};

#[derive(Debug, Serialize)]
pub struct FluxResponse<T: Serialize> {
    pub inputs: Vec<T>,
}

#[derive(Debug, Serialize)]
pub struct ClusterInfo {
    pub name: String,
    pub dns: String,
    pub environment: String,
}

#[derive(Debug, Serialize)]
pub struct SourceInfo {
    pub oci_url: String,
    pub oci_tag: String,
}

#[derive(Debug, Serialize)]
pub struct PlatformComponentInput {
    pub id: String,
    pub component_path: String,
    pub component_version: String,
    pub cluster_env_enabled: bool,
    pub depends_on: Vec<String>,
    pub enabled: bool,
    pub patches: HashMap<String, String>,
    pub cluster: ClusterInfo,
    pub source: SourceInfo,
}

#[derive(Debug, Serialize)]
pub struct NamespaceInput {
    pub id: String,
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
    pub cluster: ClusterInfo,
}

#[derive(Debug, Serialize)]
pub struct RolebindingInput {
    pub id: String,
    pub role: String,
    pub subjects: Vec<serde_json::Value>,
    pub cluster: ClusterInfo,
}

#[derive(Debug, Serialize)]
pub struct ClusterListInput {
    pub id: String,
    pub cluster_name: String,
    pub cluster_dns: String,
    pub environment: String,
}

fn cluster_info(cluster: &ClusterDoc) -> ClusterInfo {
    ClusterInfo {
        name: cluster.cluster_name.clone(),
        dns: cluster.cluster_dns.clone(),
        environment: cluster.environment.clone(),
    }
}

pub fn merge_platform_components(
    cluster: &ClusterDoc,
    catalog: &HashMap<String, ComponentCatalogDoc>,
) -> FluxResponse<PlatformComponentInput> {
    let inputs = cluster
        .platform_components
        .iter()
        .filter_map(|entry| {
            let cat = catalog.get(&entry.id)?;
            Some(PlatformComponentInput {
                id: entry.id.clone(),
                component_path: entry
                    .component_path
                    .clone()
                    .unwrap_or_else(|| cat.component_path.clone()),
                component_version: cat.component_version.clone(),
                cluster_env_enabled: cat.cluster_env_enabled,
                depends_on: cat.depends_on.clone(),
                enabled: entry.enabled,
                patches: cluster.patches.get(&entry.id).cloned().unwrap_or_default(),
                cluster: cluster_info(cluster),
                source: SourceInfo {
                    oci_url: cat.oci_url.clone(),
                    oci_tag: entry.oci_tag.clone().unwrap_or_else(|| cat.oci_tag.clone()),
                },
            })
        })
        .collect();

    FluxResponse { inputs }
}

pub fn merge_namespaces(cluster: &ClusterDoc) -> FluxResponse<NamespaceInput> {
    let inputs = cluster
        .namespaces
        .iter()
        .map(|ns| NamespaceInput {
            id: ns.id.clone(),
            labels: ns.labels.clone(),
            annotations: ns.annotations.clone(),
            cluster: cluster_info(cluster),
        })
        .collect();

    FluxResponse { inputs }
}

pub fn merge_rolebindings(cluster: &ClusterDoc) -> FluxResponse<RolebindingInput> {
    let inputs = cluster
        .rolebindings
        .iter()
        .map(|rb| RolebindingInput {
            id: rb.id.clone(),
            role: rb.role.clone(),
            subjects: rb.subjects.clone(),
            cluster: cluster_info(cluster),
        })
        .collect();

    FluxResponse { inputs }
}

pub fn merge_clusters(clusters: &[ClusterDoc]) -> FluxResponse<ClusterListInput> {
    let inputs = clusters
        .iter()
        .map(|c| ClusterListInput {
            id: c.id.clone(),
            cluster_name: c.cluster_name.clone(),
            cluster_dns: c.cluster_dns.clone(),
            environment: c.environment.clone(),
        })
        .collect();

    FluxResponse { inputs }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        ClusterComponentRef, ClusterDoc, ComponentCatalogDoc, NamespaceRef, RolebindingRef,
    };

    fn test_cluster() -> ClusterDoc {
        ClusterDoc {
            id: "test-01".into(),
            cluster_name: "test-01".into(),
            cluster_dns: "test-01.example.com".into(),
            environment: "dev".into(),
            node_count: None,
            vm_image: None,
            k0s_version: None,
            platform_components: vec![
                ClusterComponentRef {
                    id: "podinfo".into(),
                    enabled: true,
                    oci_tag: None,
                    component_path: None,
                },
                ClusterComponentRef {
                    id: "cert-manager".into(),
                    enabled: true,
                    oci_tag: Some("v2.0.0".into()),
                    component_path: None,
                },
                ClusterComponentRef {
                    id: "ingress-nginx".into(),
                    enabled: false,
                    oci_tag: None,
                    component_path: Some("custom/ingress/1.0.0".into()),
                },
            ],
            namespaces: vec![NamespaceRef {
                id: "podinfo".into(),
                labels: HashMap::from([("app".into(), "podinfo".into())]),
                annotations: HashMap::new(),
            }],
            rolebindings: vec![RolebindingRef {
                id: "admins".into(),
                role: "cluster-admin".into(),
                subjects: vec![serde_json::json!({"kind": "Group", "name": "admins"})],
            }],
            patches: HashMap::from([(
                "ingress-nginx".into(),
                HashMap::from([("REPLICAS".into(), "3".into())]),
            )]),
        }
    }

    fn test_catalog() -> HashMap<String, ComponentCatalogDoc> {
        HashMap::from([
            (
                "podinfo".into(),
                ComponentCatalogDoc {
                    id: "podinfo".into(),
                    component_path: "apps/podinfo/6.5.0".into(),
                    component_version: "6.5.0".into(),
                    cluster_env_enabled: true,
                    oci_url: "oci://ghcr.io/example/podinfo".into(),
                    oci_tag: "v1.0.0".into(),
                    depends_on: vec![],
                },
            ),
            (
                "cert-manager".into(),
                ComponentCatalogDoc {
                    id: "cert-manager".into(),
                    component_path: "core/cert-manager/1.14.0".into(),
                    component_version: "1.14.0".into(),
                    cluster_env_enabled: true,
                    oci_url: "oci://ghcr.io/example/cert-manager".into(),
                    oci_tag: "v1.0.0".into(),
                    depends_on: vec![],
                },
            ),
            (
                "ingress-nginx".into(),
                ComponentCatalogDoc {
                    id: "ingress-nginx".into(),
                    component_path: "core/ingress-nginx/4.8.0".into(),
                    component_version: "4.8.0".into(),
                    cluster_env_enabled: false,
                    oci_url: "oci://ghcr.io/example/ingress".into(),
                    oci_tag: "v1.0.0".into(),
                    depends_on: vec!["cert-manager".into()],
                },
            ),
        ])
    }

    #[test]
    fn test_catalog_defaults_used() {
        let cluster = test_cluster();
        let catalog = test_catalog();
        let resp = merge_platform_components(&cluster, &catalog);

        let podinfo = resp.inputs.iter().find(|i| i.id == "podinfo").unwrap();
        assert_eq!(podinfo.component_path, "apps/podinfo/6.5.0");
        assert_eq!(podinfo.source.oci_tag, "v1.0.0");
        assert_eq!(podinfo.source.oci_url, "oci://ghcr.io/example/podinfo");
    }

    #[test]
    fn test_cluster_oci_tag_override() {
        let cluster = test_cluster();
        let catalog = test_catalog();
        let resp = merge_platform_components(&cluster, &catalog);

        let cm = resp.inputs.iter().find(|i| i.id == "cert-manager").unwrap();
        assert_eq!(cm.source.oci_tag, "v2.0.0");
    }

    #[test]
    fn test_cluster_component_path_override() {
        let cluster = test_cluster();
        let catalog = test_catalog();
        let resp = merge_platform_components(&cluster, &catalog);

        let nginx = resp
            .inputs
            .iter()
            .find(|i| i.id == "ingress-nginx")
            .unwrap();
        assert_eq!(nginx.component_path, "custom/ingress/1.0.0");
    }

    #[test]
    fn test_patches_injection() {
        let cluster = test_cluster();
        let catalog = test_catalog();
        let resp = merge_platform_components(&cluster, &catalog);

        let nginx = resp
            .inputs
            .iter()
            .find(|i| i.id == "ingress-nginx")
            .unwrap();
        assert_eq!(nginx.patches.get("REPLICAS").unwrap(), "3");

        let podinfo = resp.inputs.iter().find(|i| i.id == "podinfo").unwrap();
        assert!(podinfo.patches.is_empty());
    }

    #[test]
    fn test_cluster_env_enabled_passthrough() {
        let cluster = test_cluster();
        let catalog = test_catalog();
        let resp = merge_platform_components(&cluster, &catalog);

        let podinfo = resp.inputs.iter().find(|i| i.id == "podinfo").unwrap();
        assert!(podinfo.cluster_env_enabled);

        let nginx = resp
            .inputs
            .iter()
            .find(|i| i.id == "ingress-nginx")
            .unwrap();
        assert!(!nginx.cluster_env_enabled);
    }

    #[test]
    fn test_depends_on_passthrough() {
        let cluster = test_cluster();
        let catalog = test_catalog();
        let resp = merge_platform_components(&cluster, &catalog);

        let nginx = resp
            .inputs
            .iter()
            .find(|i| i.id == "ingress-nginx")
            .unwrap();
        assert_eq!(nginx.depends_on, vec!["cert-manager"]);

        let podinfo = resp.inputs.iter().find(|i| i.id == "podinfo").unwrap();
        assert!(podinfo.depends_on.is_empty());
    }

    #[test]
    fn test_missing_catalog_entry_skipped() {
        let mut cluster = test_cluster();
        cluster.platform_components.push(ClusterComponentRef {
            id: "nonexistent".into(),
            enabled: true,
            oci_tag: None,
            component_path: None,
        });
        let catalog = test_catalog();
        let resp = merge_platform_components(&cluster, &catalog);
        assert_eq!(resp.inputs.len(), 3);
    }

    #[test]
    fn test_empty_component_list() {
        let mut cluster = test_cluster();
        cluster.platform_components.clear();
        let catalog = test_catalog();
        let resp = merge_platform_components(&cluster, &catalog);
        assert!(resp.inputs.is_empty());
    }

    #[test]
    fn test_cluster_info_nested() {
        let cluster = test_cluster();
        let catalog = test_catalog();
        let resp = merge_platform_components(&cluster, &catalog);
        let first = &resp.inputs[0];
        assert_eq!(first.cluster.name, "test-01");
        assert_eq!(first.cluster.dns, "test-01.example.com");
        assert_eq!(first.cluster.environment, "dev");
    }

    #[test]
    fn test_merge_namespaces() {
        let cluster = test_cluster();
        let resp = merge_namespaces(&cluster);
        assert_eq!(resp.inputs.len(), 1);
        assert_eq!(resp.inputs[0].id, "podinfo");
        assert_eq!(resp.inputs[0].labels.get("app").unwrap(), "podinfo");
        assert_eq!(resp.inputs[0].cluster.name, "test-01");
    }

    #[test]
    fn test_merge_rolebindings() {
        let cluster = test_cluster();
        let resp = merge_rolebindings(&cluster);
        assert_eq!(resp.inputs.len(), 1);
        assert_eq!(resp.inputs[0].id, "admins");
        assert_eq!(resp.inputs[0].role, "cluster-admin");
        assert_eq!(resp.inputs[0].subjects[0]["name"], "admins");
    }

    #[test]
    fn test_merge_clusters() {
        let clusters = vec![test_cluster()];
        let resp = merge_clusters(&clusters);
        assert_eq!(resp.inputs.len(), 1);
        assert_eq!(resp.inputs[0].id, "test-01");
        assert_eq!(resp.inputs[0].cluster_dns, "test-01.example.com");
    }

    #[test]
    fn test_both_overrides() {
        let mut cluster = test_cluster();
        cluster.platform_components = vec![ClusterComponentRef {
            id: "podinfo".into(),
            enabled: true,
            oci_tag: Some("v9.9.9".into()),
            component_path: Some("custom/path".into()),
        }];
        let catalog = test_catalog();
        let resp = merge_platform_components(&cluster, &catalog);
        assert_eq!(resp.inputs[0].source.oci_tag, "v9.9.9");
        assert_eq!(resp.inputs[0].component_path, "custom/path");
    }
}
