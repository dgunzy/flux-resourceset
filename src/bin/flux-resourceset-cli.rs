use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use flux_resourceset::apis::configuration::Configuration;
use flux_resourceset::{apis, models};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};

#[derive(Parser, Debug)]
#[command(name = "flux-resourceset-cli")]
#[command(about = "CLI for flux-resourceset CRUD and demo workflows")]
struct Cli {
    #[arg(long, env = "FLUX_API_URL", default_value = "http://127.0.0.1:8080")]
    api_url: String,

    #[arg(long, env = "FLUX_API_TOKEN")]
    api_token: String,

    #[arg(long, env = "FLUX_API_WRITE_TOKEN")]
    write_token: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Cluster {
        #[command(subcommand)]
        command: ClusterCommands,
    },
    Namespace {
        #[command(subcommand)]
        command: NamespaceCommands,
    },
    Demo {
        #[command(subcommand)]
        command: DemoCommands,
    },
}

#[derive(Subcommand, Debug)]
enum ClusterCommands {
    List,
    Get { cluster_id: String },
}

#[derive(Subcommand, Debug)]
enum NamespaceCommands {
    List,
    Get {
        namespace_id: String,
    },
    Create {
        namespace_id: String,
        #[arg(long = "label")]
        labels: Vec<String>,
        #[arg(long = "annotation")]
        annotations: Vec<String>,
    },
    Delete {
        namespace_id: String,
    },
}

#[derive(Subcommand, Debug)]
enum DemoCommands {
    AddNamespace {
        cluster_id: String,
        namespace_id: String,
        #[arg(long = "label")]
        labels: Vec<String>,
        #[arg(long = "annotation")]
        annotations: Vec<String>,
    },
    FluxNamespaces {
        cluster_dns: String,
    },
    PatchComponent {
        cluster_id: String,
        component_id: String,
        #[arg(long = "set")]
        values: Vec<String>,
    },
}

#[derive(Clone)]
struct ApiClients {
    read: Configuration,
    write: Configuration,
}

impl ApiClients {
    fn new(api_url: &str, read_token: &str, write_token: Option<&str>) -> Result<Self> {
        let read = configuration_with_auth(api_url, read_token)?;
        let write = configuration_with_auth(api_url, write_token.unwrap_or(read_token))?;
        Ok(Self { read, write })
    }
}

fn configuration_with_auth(api_url: &str, token: &str) -> Result<Configuration> {
    let mut headers = HeaderMap::new();
    let auth_header = HeaderValue::from_str(&format!("Bearer {token}"))
        .context("failed to construct authorization header")?;
    headers.insert(AUTHORIZATION, auth_header);

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .context("failed to build API client")?;

    let mut config = Configuration::new();
    config.base_path = api_url.to_string();
    config.bearer_access_token = Some(token.to_string());
    config.client = client;
    Ok(config)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let clients = ApiClients::new(&cli.api_url, &cli.api_token, cli.write_token.as_deref())?;

    match cli.command {
        Commands::Cluster { command } => handle_cluster(&clients, command).await,
        Commands::Namespace { command } => handle_namespace(&clients, command).await,
        Commands::Demo { command } => handle_demo(&clients, &cli.api_url, command).await,
    }
}

async fn handle_cluster(clients: &ApiClients, command: ClusterCommands) -> Result<()> {
    match command {
        ClusterCommands::List => {
            let rows =
                apis::clusters_api::clusters_get(&clients.read, None, None, None, None).await?;
            print_json(&rows)
        }
        ClusterCommands::Get { cluster_id } => {
            let row =
                apis::clusters_api::clusters_id_get(&clients.read, &cluster_id, None, None).await?;
            print_json(&row)
        }
    }
}

async fn handle_namespace(clients: &ApiClients, command: NamespaceCommands) -> Result<()> {
    match command {
        NamespaceCommands::List => {
            let rows = apis::namespaces_api::namespaces_get(&clients.read, None, None).await?;
            print_json(&rows)
        }
        NamespaceCommands::Get { namespace_id } => {
            let row = apis::namespaces_api::namespaces_id_get(&clients.read, &namespace_id).await?;
            print_json(&row)
        }
        NamespaceCommands::Create {
            namespace_id,
            labels,
            annotations,
        } => {
            let labels = parse_key_values(&labels)?;
            let annotations = parse_key_values(&annotations)?;
            let row = upsert_namespace(clients, &namespace_id, labels, annotations).await?;
            print_json(&row)
        }
        NamespaceCommands::Delete { namespace_id } => {
            let row =
                apis::namespaces_api::namespaces_id_delete(&clients.write, &namespace_id).await?;
            print_json(&row)
        }
    }
}

async fn handle_demo(clients: &ApiClients, api_url: &str, command: DemoCommands) -> Result<()> {
    match command {
        DemoCommands::AddNamespace {
            cluster_id,
            namespace_id,
            labels,
            annotations,
        } => {
            let labels = parse_key_values(&labels)?;
            let annotations = parse_key_values(&annotations)?;

            let namespace =
                upsert_namespace(clients, &namespace_id, labels.clone(), annotations.clone())
                    .await?;

            let mut cluster =
                apis::clusters_api::clusters_id_get(&clients.read, &cluster_id, None, None).await?;

            let mut namespaces = cluster.namespaces.take().unwrap_or_default();
            if let Some(existing) = namespaces.iter_mut().find(|ns| ns.id == namespace_id) {
                existing.labels = Some(labels.clone());
                existing.annotations = Some(annotations.clone());
            } else {
                namespaces.push(models::ClusterNamespacesInner {
                    id: namespace_id,
                    labels: Some(labels.clone()),
                    annotations: Some(annotations.clone()),
                });
            }

            let mut update = models::UpdateCluster::new();
            update.namespaces = Some(namespaces);

            apis::clusters_api::clusters_id_put(&clients.write, update, &cluster_id)
                .await
                .context("failed to update cluster namespace assignments")?;
            let updated =
                apis::clusters_api::clusters_id_get(&clients.read, &cluster_id, None, None).await?;

            println!("# Namespace upsert result");
            print_json(&namespace)?;
            println!("# Cluster update result");
            print_json(&updated)
        }
        DemoCommands::FluxNamespaces { cluster_dns } => {
            let response = reqwest::Client::new()
                .get(format!(
                    "{api_url}/api/v2/flux/clusters/{cluster_dns}/namespaces"
                ))
                .bearer_auth(clients.read.bearer_access_token.clone().unwrap_or_default())
                .send()
                .await
                .context("failed to call flux namespaces endpoint")?;

            let status = response.status();
            let text = response.text().await?;
            if !status.is_success() {
                bail!("flux endpoint returned {}: {}", status, text);
            }

            let value: serde_json::Value = serde_json::from_str(&text)?;
            print_json(&value)
        }
        DemoCommands::PatchComponent {
            cluster_id,
            component_id,
            values,
        } => {
            if values.is_empty() {
                bail!("no changes requested; provide at least one --set key=value pair");
            }
            let values = parse_key_values(&values)?;

            let before =
                apis::clusters_api::clusters_id_get(&clients.read, &cluster_id, None, None).await?;

            let mut update = models::UpdateCluster::new();
            let mut patches = before.patches.clone().unwrap_or_default();
            {
                let component_values = patches.entry(component_id.clone()).or_default();
                for (key, value) in values {
                    component_values.insert(key, value);
                }
            }
            update.patches = Some(patches);

            apis::clusters_api::clusters_id_put(&clients.write, update, &cluster_id)
                .await
                .context("failed to patch cluster component values")?;

            let after =
                apis::clusters_api::clusters_id_get(&clients.read, &cluster_id, None, None).await?;

            let change_summary = serde_json::json!({
                "cluster_id": cluster_id,
                "component_id": component_id,
                "before": {
                    "environment": before.environment,
                    "component_patches": before
                        .patches
                        .as_ref()
                        .and_then(|p| p.get(&component_id))
                },
                "after": {
                    "environment": after.environment,
                    "component_patches": after
                        .patches
                        .as_ref()
                        .and_then(|p| p.get(&component_id))
                }
            });

            print_json(&change_summary)
        }
    }
}

async fn upsert_namespace(
    clients: &ApiClients,
    namespace_id: &str,
    labels: HashMap<String, String>,
    annotations: HashMap<String, String>,
) -> Result<models::Namespace> {
    let create = models::CreateNamespace {
        id: Some(serde_json::Value::String(namespace_id.to_string())),
        labels: Some(labels.clone()),
        annotations: Some(annotations.clone()),
    };

    match apis::namespaces_api::namespaces_post(&clients.write, create, None, None).await {
        Ok(_) => {
            let namespace =
                apis::namespaces_api::namespaces_id_get(&clients.read, namespace_id).await?;
            Ok(namespace)
        }
        Err(apis::Error::ResponseError(content)) if content.status.as_u16() == 409 => {
            let mut update = models::UpdateNamespace::new();
            update.labels = Some(labels);
            update.annotations = Some(annotations);
            apis::namespaces_api::namespaces_id_put(&clients.write, update, namespace_id).await?;
            let namespace =
                apis::namespaces_api::namespaces_id_get(&clients.read, namespace_id).await?;
            Ok(namespace)
        }
        Err(err) => Err(err.into()),
    }
}

fn parse_key_values(values: &[String]) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    for value in values {
        let Some((key, val)) = value.split_once('=') else {
            bail!("invalid key-value pair '{value}', expected key=value");
        };
        if key.is_empty() {
            bail!("invalid key-value pair '{value}', key cannot be empty");
        }
        map.insert(key.to_string(), val.to_string());
    }
    Ok(map)
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
