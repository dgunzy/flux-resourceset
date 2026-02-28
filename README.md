# flux-resourceset

`flux-resourceset` is a Flux-facing external service for the `ResourceSetInputProvider` `type: ExternalService` pattern.

It demonstrates a phone-home model where each cluster pulls its own desired state from API endpoints returning `{"inputs":[...]}` payloads, then renders local `ResourceSet` templates.

## Dependencies

Core tools:

- Rust/Cargo (build and run API + CLI)
- `kubectl`
- `flux` CLI
- Docker
- kind
- `curl`

Helpful tools:

- `jq` (pretty JSON output)
- Poetry + Python 3 (for `make generate`)
- `openapi-generator` (for Rust model/client generation in `make generate`)

## What This Project Is About

- External-service GitOps: Flux in each cluster polls this API instead of being pushed from a central management cluster.
- Cluster-scoped desired state: inputs are resolved by `cluster_dns` so each cluster gets only its resources.
- Dynamic updates: operators can change cluster/platform/namespace/rolebinding state through a CRUD API and see Flux reconcile.
- Production shape: two runtime modes in one binary (`read-only` and `crud`) so read traffic can scale separately from writes.
- Normalized data model: clusters store references (`id`) to namespace/rolebinding definitions, while platform component refs can include per-cluster overrides (`enabled`, `oci_tag`, `component_path`).

## ExternalService Flow

1. A cluster runs Flux Operator with `ResourceSetInputProvider` objects (`type: ExternalService`).
2. The providers call `flux-resourceset` endpoints such as:
   - `/api/v2/flux/clusters/{cluster_dns}/platform-components`
   - `/api/v2/flux/clusters/{cluster_dns}/namespaces`
   - `/api/v2/flux/clusters/{cluster_dns}/rolebindings`
3. The API responds with `{"inputs":[...]}`.
4. Flux `ResourceSet` templates render and reconcile resources locally in that cluster.

## Quick Start (Demo Cluster)

```bash
make demo
```

The demo installs Flux Operator, deploys this API, and applies providers/resourcesets for `cert-manager`, `traefik`, `podinfo`, namespaces, and rolebindings.

## CLI Against Demo Cluster (Fast Path)

Prerequisite: run `make demo` first. All CLI steps below assume the demo cluster, API deployment, providers, and resourcesets already exist.

1. Port-forward the API:

```bash
make cli-demo-port-forward
```

2. In another terminal, build the CLI once:

```bash
cargo build --bin flux-resourceset-cli
```

3. Export env vars from the demo cluster token secret:

```bash
export FLUX_API_URL=http://127.0.0.1:8080
export FLUX_API_TOKEN="$(kubectl -n flux-system get secret internal-api-token -o jsonpath='{.data.token}' | base64 -d)"
export FLUX_API_WRITE_TOKEN="$FLUX_API_TOKEN"
```

4. Use the compiled debug binary directly:

```bash
./target/debug/flux-resourceset-cli cluster list | jq .
./target/debug/flux-resourceset-cli namespace list | jq .
./target/debug/flux-resourceset-cli demo flux-namespaces demo-cluster-01.k8s.example.com | jq .
```

If `jq` is not installed, run the same commands without `| jq .`.

Optional: run the automated demo flow (`jq` aware, reconcile + wait included):

```bash
make cli-demo
```

## Manual Namespace Workflow (No Makefile)

Use this if you want to create a namespace with your own name and attach it to the demo cluster in one command.

```bash
./target/debug/flux-resourceset-cli namespace create your-team-namespace --cluster demo-cluster-01
```

Force Flux to refresh and verify it was created:

```bash
kubectl annotate resourcesetinputprovider namespaces -n flux-system \
  fluxcd.controlplane.io/requestedAt="$(date -u +"%Y-%m-%dT%H:%M:%SZ")" --overwrite
kubectl annotate resourceset namespaces -n flux-system \
  fluxcd.controlplane.io/requestedAt="$(date -u +"%Y-%m-%dT%H:%M:%SZ")" --overwrite
kubectl get ns your-team-namespace
```

## Component Patch/Reconcile Demo

This is the core concept demo: patch component values in cluster JSON through the CLI, let Flux reconcile, then verify runtime changes.

Prerequisites:

- `make demo` has completed
- API is reachable on `http://127.0.0.1:8080` (for example `make cli-demo-port-forward`)
- CLI binary is built (`cargo build --bin flux-resourceset-cli`)

1. Check current rendered state:

```bash
kubectl get configmap -n flux-system values-podinfo-demo-cluster-01 \
  -o jsonpath='replicas={.data.replicaCount} color={.data.ui\.color} message={.data.ui\.message}{"\n"}'
kubectl get deploy -n podinfo podinfo \
  -o jsonpath='replicas={.spec.replicas} color={.spec.template.spec.containers[0].env[?(@.name=="PODINFO_UI_COLOR")].value} message={.spec.template.spec.containers[0].env[?(@.name=="PODINFO_UI_MESSAGE")].value}{"\n"}'
```

2. Patch component values in the API-backed cluster schema:

```bash
./target/debug/flux-resourceset-cli demo patch-component demo-cluster-01 podinfo \
  --set replicaCount=3 \
  --set ui.message="Hello from $(whoami) via CLI patch" \
  --set ui.color="#3b82f6" | jq .
```

The command prints `before` and `after` values for the component patch map.
Patch keys are dynamic string paths; dotted keys (for example `ui.message`) map to nested Helm values.

3. Reconcile fast and watch updates:

```bash
# Refresh Flux Operator inputs and rendered resources
kubectl annotate resourcesetinputprovider platform-components -n flux-system \
  fluxcd.controlplane.io/requestedAt="$(date -u +"%Y-%m-%dT%H:%M:%SZ")" --overwrite
kubectl annotate resourceset platform-components -n flux-system \
  fluxcd.controlplane.io/requestedAt="$(date -u +"%Y-%m-%dT%H:%M:%SZ")" --overwrite

# Trigger immediate Helm upgrade
flux reconcile helmrelease platform-podinfo -n flux-system --with-source

kubectl get hr -n flux-system platform-podinfo \
  -o jsonpath='ready={.status.conditions[?(@.type=="Ready")].status} reason={.status.conditions[?(@.type=="Ready")].reason} action={.status.lastAttemptedReleaseAction}{"\n"}'
```

4. Validate the change:

```bash
kubectl get configmap -n flux-system values-podinfo-demo-cluster-01 \
  -o jsonpath='replicas={.data.replicaCount} color={.data.ui\.color} message={.data.ui\.message}{"\n"}'
kubectl get deploy -n podinfo podinfo \
  -o jsonpath='replicas={.spec.replicas} color={.spec.template.spec.containers[0].env[?(@.name=="PODINFO_UI_COLOR")].value} message={.spec.template.spec.containers[0].env[?(@.name=="PODINFO_UI_MESSAGE")].value}{"\n"}'
```

Optional UI check in browser:

```bash
kubectl -n podinfo port-forward svc/podinfo 9898:9898
```

Then open `http://127.0.0.1:9898` and confirm the UI message/color changed.

## Runtime Modes

- `read-only`: multi-replica Flux polling API only (`/api/v2/flux/...`).
- `crud`: full CRUD API (`/clusters`, `/platform_components`, `/namespaces`, `/rolebindings`) plus read endpoints.

Run read-only:

```bash
export API_MODE=read-only
export AUTH_TOKEN=dev-token
cargo run
```

Run CRUD:

```bash
export API_MODE=crud
export AUTH_TOKEN=read-token
export CRUD_AUTH_TOKEN=write-token
cargo run
```

## Firestone Code Generation

`flux-resourceset` keeps Firestone resource schemas as the API source of truth.

Generated outputs:

- OpenAPI spec: `openapi/openapi.yaml`
- Rust models: `src/models/`
- Rust API client modules: `src/apis/`
- Rust CLI modules: `src/generated/cli/`

Generate:

```bash
make generate
```

## Endpoints

Flux read endpoints:

- `GET /api/v2/flux/clusters/{cluster_dns}/platform-components`
- `GET /api/v2/flux/clusters/{cluster_dns}/namespaces`
- `GET /api/v2/flux/clusters/{cluster_dns}/rolebindings`
- `GET /api/v2/flux/clusters`

CRUD endpoints (`API_MODE=crud`):

- `GET, POST /clusters`
- `GET, PUT, DELETE /clusters/{id}`
- `GET, POST /platform_components`
- `GET, PUT, DELETE /platform_components/{id}`
- `GET, POST /namespaces`
- `GET, PUT, DELETE /namespaces/{id}`
- `GET, POST /rolebindings`
- `GET, PUT, DELETE /rolebindings/{id}`

Service endpoints:

- `GET /health`
- `GET /ready`
- `GET /openapi.yaml`

## Configuration

| Variable | Required | Default | Description |
| --- | --- | --- | --- |
| `API_MODE` | no | `read-only` | Runtime mode: `read-only` or `crud` |
| `STORE_BACKEND` | no | `sqlite` | Data backend: `sqlite` or `memory` |
| `DATABASE_URL` | no | `sqlite://data/flux-resourceset.db?mode=rwc` | SQLite DSN when `STORE_BACKEND=sqlite` |
| `AUTH_TOKEN` | yes | none | Bearer token for read routes |
| `CRUD_AUTH_TOKEN` | no | `AUTH_TOKEN` | Bearer token for write routes in CRUD mode |
| `SEED_FILE` | no | `data/seed.json` | Seed data file loaded at startup |
| `OPENAPI_FILE` | no | `openapi/openapi.yaml` | OpenAPI document served at `/openapi.yaml` |
| `LISTEN_ADDR` | no | `0.0.0.0:8080` | Bind address |
| `RUST_LOG` | no | unset | Tracing filter |

## Development

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```
