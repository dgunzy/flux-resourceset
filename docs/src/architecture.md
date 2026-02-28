# System Overview

The architecture separates concerns into three layers: the **data plane** (where cluster config lives), the **API plane** (this service), and the **cluster plane** (Flux running on each child cluster).

## High-Level Architecture

```mermaid
graph TB
    subgraph "Data Layer"
        DB[("Data Store<br/>(SQLite / In-Memory)")]
    end

    subgraph "API Layer"
        READ["flux-resourceset<br/>(read-only mode)"]
        CRUD["flux-resourceset<br/>(CRUD mode)"]
        CLI["flux-resourceset-cli"]
    end

    subgraph "Cluster Layer"
        subgraph "Child Cluster"
            RSIP["ResourceSetInputProvider<br/>type: ExternalService"]
            RS["ResourceSet<br/>(templates)"]
            HR["HelmRelease / Kustomization"]
            NS["Namespace"]
            RB["ClusterRoleBinding"]
        end
    end

    DB -->|"read"| READ
    DB <-->|"read/write"| CRUD
    CLI -->|"CRUD operations"| CRUD
    RSIP -->|"polls"| READ
    RSIP -->|"inputs"| RS
    RS -->|"renders"| HR
    RS -->|"renders"| NS
    RS -->|"renders"| RB
```

## Component Roles

### Data Store

By default, this is SQLite (configured via `DATABASE_URL`). For lightweight/dev workflows it can run in-memory (`STORE_BACKEND=memory`) using `data/seed.json` as initial state.

The store holds four logical resource sets:

- **clusters** — each cluster's full configuration: assigned components, namespaces, rolebindings, and per-component patches
- **platform_components** — component catalog entries with defaults, OCI URLs/tags, and dependencies
- **namespaces** — reusable namespace definitions referenced by clusters
- **rolebindings** — reusable RBAC rolebinding definitions referenced by clusters

### API Service (flux-resourceset)

A Rust service built with axum that operates in two modes:

| Mode | Purpose | Endpoints |
|------|---------|-----------|
| `read-only` | Flux polling — high concurrency, minimal resource usage | `/api/v2/flux/...`, `/health`, `/ready` |
| `crud` | Operator/CLI access — full CRUD for managing cluster state | All read endpoints + `/clusters`, `/platform_components`, `/namespaces`, `/rolebindings` |

The read-only mode is designed to run as a multi-replica deployment serving cluster polls. The CRUD mode is for operators and CI/CD pipelines that need to modify cluster configuration.

### CLI (flux-resourceset-cli)

A command-line tool for interacting with the CRUD API. Supports listing, creating, and patching resources. Used for demos and operational workflows.

### Flux Operator (on each cluster)

Each cluster runs:

1. **ResourceSetInputProvider** — calls the API on a schedule, fetches `{"inputs": [...]}`
2. **ResourceSet** — takes the inputs and renders Kubernetes manifests from templates
3. **Flux controllers** — reconcile the rendered manifests (HelmRelease, Kustomization, Namespace, etc.)

## Data Flow

```mermaid
sequenceDiagram
    participant Operator as Operator / CLI
    participant API as flux-resourceset (CRUD)
    participant DB as Data Store
    participant ReadAPI as flux-resourceset (read-only)
    participant Cluster as Child Cluster (Flux)

    Operator->>API: PATCH /clusters/demo-cluster-01<br/>{"patches": {"podinfo": {"replicaCount": "3"}}}
    API->>DB: Update cluster document
    API-->>Operator: 200 OK

    Note over Cluster: Every 5 minutes (or on-demand)

    Cluster->>ReadAPI: GET /api/v2/flux/clusters/{dns}/platform-components
    ReadAPI->>DB: Fetch cluster + catalog docs
    DB-->>ReadAPI: Cluster doc + component catalog
    ReadAPI->>ReadAPI: Merge overrides with catalog defaults
    ReadAPI-->>Cluster: {"inputs": [{...component with patches...}]}

    Cluster->>Cluster: ResourceSet renders HelmRelease with patched values
    Cluster->>Cluster: Flux reconciles — podinfo scales to 3 replicas
```

## Why This Architecture

### vs. Push-Based (ArgoCD ApplicationSets, central Flux)

| Concern | Push-based | Phone-home (this) |
|---------|-----------|-------------------|
| **Scalability** | Management cluster must maintain connections to all children | Each cluster independently polls; API is stateless |
| **Failure blast radius** | Management cluster outage = all clusters lose reconciliation | API outage = clusters keep running last-known state |
| **Network requirements** | Management cluster needs outbound access to all clusters | Clusters need outbound access to one API endpoint |
| **Credential management** | Management cluster holds kubeconfigs for all clusters | Each cluster holds one bearer token |

### vs. Git-per-Cluster

| Concern | Git-per-cluster | API-driven (this) |
|---------|-----------------|--------------------|
| **Updating 500 clusters** | 500 PRs or complex monorepo tooling | One API call to update the component catalog |
| **Per-cluster overrides** | Branch strategies or overlay directories | First-class `patches` object per cluster |
| **Audit trail** | Git history | API audit log + Git history for templates |
| **Dynamic response** | Static YAML files | Merge logic computes cluster-specific state |

### vs. Direct Kubernetes API Access

A common question is: why not have operators `kubectl apply` directly, or build tooling that talks to the Kubernetes API on each cluster? See the [FAQ](./faq.md#why-an-api-instead-of-direct-kubernetes-api-access) for a detailed answer. The short version: a purpose-built API gives you a single control point with business logic, validation, audit logging, and integration hooks — things the raw Kubernetes API does not provide at fleet scale.

## Infrastructure Agnostic

This architecture has no dependency on a specific cloud provider, VM provisioner, or Kubernetes distribution. The phone-home pattern requires only one thing: **outbound HTTPS from each cluster to the API**.

```mermaid
graph TB
    API["flux-resourceset API"]

    subgraph "On-Premises Data Center"
        OP1["Bare-metal cluster"]
        OP2["VMware vSphere cluster"]
    end

    subgraph "Public Cloud"
        AWS["AWS EKS"]
        AZ["Azure AKS"]
        GCP["GCP GKE"]
    end

    subgraph "Edge"
        E1["Edge location 1"]
        E2["Edge location 2"]
    end

    OP1 & OP2 -->|"HTTPS"| API
    AWS & AZ & GCP -->|"HTTPS"| API
    E1 & E2 -->|"HTTPS"| API
```

| Environment | How It Works |
|-------------|-------------|
| **On-prem bare metal** | Clusters provisioned via PXE boot, cloud-init, or immutable OS images. Flux bootstrap manifests pre-installed or applied post-boot. |
| **On-prem VMs** | VMware, KVM, Hyper-V, or any hypervisor. Same bootstrap pattern — inject identity, let Flux phone home. |
| **Public cloud managed K8s** | EKS, AKS, GKE — deploy Flux Operator as an add-on or Helm chart. Providers and ResourceSets applied via GitOps or cluster bootstrap. |
| **Edge / remote sites** | Lightweight clusters (k3s, k0s, MicroK8s) at edge locations. Phone home over VPN or direct HTTPS. |
| **Hybrid** | Mix any of the above. Each cluster phones home to the same API regardless of where it runs. |

The cluster provisioning mechanism is completely decoupled from the platform component management. Whether you use Terraform, Crossplane, Cluster API, custom scripts, or manual provisioning — once Flux is running and the cluster-identity ConfigMap exists, the phone-home loop takes over.
