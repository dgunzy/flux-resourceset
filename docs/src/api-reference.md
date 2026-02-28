# API Reference

All endpoints return JSON. Flux-facing endpoints return the `{"inputs": [...]}` structure required by the ResourceSetInputProvider ExternalService contract. CRUD endpoints follow standard REST conventions.

## Authentication

All endpoints require a `Bearer` token in the `Authorization` header.

| Mode | Read Token | Write Token |
|------|-----------|-------------|
| `read-only` | `AUTH_TOKEN` env var | N/A (no write endpoints) |
| `crud` | `AUTH_TOKEN` env var | `CRUD_AUTH_TOKEN` env var (falls back to `AUTH_TOKEN`) |

```bash
curl -H "Authorization: Bearer $AUTH_TOKEN" http://localhost:8080/health
```

---

## Flux Read Endpoints

These endpoints are consumed by Flux Operator's ResourceSetInputProvider. They follow the ExternalService contract.

### ExternalService Contract

Every response must satisfy:

- Top-level `inputs` array
- Each item has a unique string `id`
- Response body under 900 KiB
- All JSON value types (strings, numbers, booleans, arrays, objects) are preserved in templates

### `GET /api/v2/flux/clusters/{cluster_dns}/platform-components`

Returns platform components assigned to a cluster, with catalog defaults merged and per-cluster overrides applied.

**Path parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `cluster_dns` | string | The cluster's DNS name (e.g., `demo-cluster-01.k8s.example.com`) |

**Response:**

```json
{
  "inputs": [
    {
      "id": "cert-manager",
      "component_path": "cert-manager",
      "component_version": "latest",
      "cluster_env_enabled": false,
      "depends_on": [],
      "enabled": true,
      "patches": {},
      "cluster": {
        "name": "demo-cluster-01",
        "dns": "demo-cluster-01.k8s.example.com",
        "environment": "dev"
      },
      "source": {
        "oci_url": "https://charts.jetstack.io",
        "oci_tag": "latest"
      }
    }
  ]
}
```

**Field reference:**

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique component identifier, used as Flux resource name suffix |
| `component_path` | string | Chart name or path within OCI artifact. Cluster override takes precedence over catalog default |
| `component_version` | string | Upstream version. `"latest"` means no version pinning |
| `cluster_env_enabled` | boolean | If `true`, ResourceSet template appends `/{environment}` to the path |
| `depends_on` | string[] | Component IDs that must be healthy first. Empty = no dependencies |
| `enabled` | boolean | `false` causes Flux to garbage-collect the component |
| `patches` | object | Per-cluster key-value overrides, injected via HelmRelease `valuesFrom` |
| `cluster.name` | string | Cluster identifier |
| `cluster.dns` | string | Cluster FQDN |
| `cluster.environment` | string | Tier: `dev`, `qa`, `uat`, `prod` |
| `source.oci_url` | string | Helm repository or OCI registry URL |
| `source.oci_tag` | string | Chart/artifact version tag. Cluster override takes precedence |

### `GET /api/v2/flux/clusters/{cluster_dns}/namespaces`

Returns namespaces assigned to a cluster.

**Response:**

```json
{
  "inputs": [
    {
      "id": "cert-manager",
      "labels": { "app": "cert-manager" },
      "annotations": {},
      "cluster": {
        "name": "demo-cluster-01",
        "dns": "demo-cluster-01.k8s.example.com",
        "environment": "dev"
      }
    }
  ]
}
```

### `GET /api/v2/flux/clusters/{cluster_dns}/rolebindings`

Returns role bindings assigned to a cluster.

**Response:**

```json
{
  "inputs": [
    {
      "id": "platform-admins",
      "role": "cluster-admin",
      "subjects": [
        {
          "kind": "Group",
          "name": "platform-team",
          "apiGroup": "rbac.authorization.k8s.io"
        }
      ],
      "cluster": {
        "name": "demo-cluster-01",
        "dns": "demo-cluster-01.k8s.example.com",
        "environment": "dev"
      }
    }
  ]
}
```

### `GET /api/v2/flux/clusters`

Returns all clusters. Used by management cluster provisioners.

**Response:**

```json
{
  "inputs": [
    {
      "id": "demo-cluster-01",
      "cluster_name": "demo-cluster-01",
      "cluster_dns": "demo-cluster-01.k8s.example.com",
      "environment": "dev"
    }
  ]
}
```

---

## CRUD Endpoints

Available when `API_MODE=crud`. These follow standard REST patterns.

### Clusters

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/clusters` | List all clusters |
| `POST` | `/clusters` | Create a cluster |
| `GET` | `/clusters/{id}` | Get cluster by ID |
| `PUT` | `/clusters/{id}` | Update a cluster |
| `DELETE` | `/clusters/{id}` | Delete a cluster |

Cluster payload notes:

- `platform_components[]` entries are references with per-cluster override fields (`id`, `enabled`, optional `oci_tag`, optional `component_path`).
- `namespaces[]` entries are reference objects (`id` only).
- `rolebindings[]` entries are reference objects (`id` only).

### Platform Components

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/platform_components` | List all catalog components |
| `POST` | `/platform_components` | Create a catalog entry |
| `GET` | `/platform_components/{id}` | Get component by ID |
| `PUT` | `/platform_components/{id}` | Update a catalog entry |
| `DELETE` | `/platform_components/{id}` | Delete a catalog entry |

### Namespaces

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/namespaces` | List all namespace definitions |
| `POST` | `/namespaces` | Create a namespace definition |
| `GET` | `/namespaces/{id}` | Get namespace by ID |
| `PUT` | `/namespaces/{id}` | Update a namespace definition |
| `DELETE` | `/namespaces/{id}` | Delete a namespace definition |

### Rolebindings

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/rolebindings` | List all rolebinding definitions |
| `POST` | `/rolebindings` | Create a rolebinding definition |
| `GET` | `/rolebindings/{id}` | Get rolebinding by ID |
| `PUT` | `/rolebindings/{id}` | Update a rolebinding definition |
| `DELETE` | `/rolebindings/{id}` | Delete a rolebinding definition |

---

## Service Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/health` | Liveness probe — returns `{"status": "ok"}` |
| `GET` | `/ready` | Readiness probe endpoint — currently returns `{"status": "ok"}` |
| `GET` | `/openapi.yaml` | OpenAPI 3.0 specification document |

---

## Error Responses

| Status | Condition |
|--------|-----------|
| `401 Unauthorized` | Missing or invalid bearer token |
| `404 Not Found` | Cluster DNS or resource ID not found |
| `500 Internal Server Error` | Data store connection error |
