# CLI Usage

`flux-resourceset-cli` is a command-line tool for interacting with the CRUD API. It is built from the same codebase and generated from the same Firestone schemas as the API.

## Building

```bash
cd flux-resourceset
cargo build --bin flux-resourceset-cli
```

The binary is at `target/debug/flux-resourceset-cli`.

## Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `FLUX_API_URL` | yes | API base URL (e.g., `http://127.0.0.1:8080`) |
| `FLUX_API_TOKEN` | yes | Bearer token for read operations |
| `FLUX_API_WRITE_TOKEN` | yes | Bearer token for write operations |

### Setup from Demo Cluster

```bash
export FLUX_API_URL=http://127.0.0.1:8080
export FLUX_API_TOKEN="$(kubectl -n flux-system get secret internal-api-token \
  -o jsonpath='{.data.token}' | base64 -d)"
export FLUX_API_WRITE_TOKEN="$FLUX_API_TOKEN"
```

## Commands

### Cluster Operations

```bash
# List all clusters
flux-resourceset-cli cluster list

# Get a specific cluster
flux-resourceset-cli cluster get demo-cluster-01
```

### Namespace Operations

```bash
# List all namespaces
flux-resourceset-cli namespace list

# Get a specific namespace
flux-resourceset-cli namespace get cert-manager
```

### Platform Component Operations

```bash
# List all catalog components
flux-resourceset-cli platform-component list

# Get a specific component
flux-resourceset-cli platform-component get cert-manager
```

### Rolebinding Operations

```bash
# List all rolebindings
flux-resourceset-cli rolebinding list
```

### Demo Commands

The CLI includes demo-specific commands for common workflows:

```bash
# Add a namespace to a cluster
flux-resourceset-cli demo add-namespace <cluster-id> <namespace> \
  --label team=platform \
  --annotation owner=you

# Patch one component using dynamic key/value paths
flux-resourceset-cli demo patch-component <cluster-id> <component-id> \
  --set replicaCount=3 \
  --set ui.message="Hello" \
  --set ui.color="#3b82f6"

# Get Flux-formatted namespace response
flux-resourceset-cli demo flux-namespaces <cluster-dns>
```

## Output

All CLI commands output JSON. Pipe to `jq` for pretty formatting:

```bash
flux-resourceset-cli cluster list | jq .
```

## Workflow Examples

### Add a namespace and watch Flux create it

```bash
# 1. Add the namespace
flux-resourceset-cli demo add-namespace demo-cluster-01 team-sandbox \
  --label team=sandbox --annotation owner=platform

# 2. Force reconcile
kubectl annotate resourcesetinputprovider namespaces -n flux-system \
  fluxcd.controlplane.io/requestedAt="$(date -u +"%Y-%m-%dT%H:%M:%SZ")" --overwrite

# 3. Wait and verify
kubectl get ns team-sandbox
```

### Patch a component and verify

```bash
# 1. Patch
flux-resourceset-cli demo patch-component demo-cluster-01 podinfo --set replicaCount=5

# 2. Force reconcile
kubectl annotate resourcesetinputprovider platform-components -n flux-system \
  fluxcd.controlplane.io/requestedAt="$(date -u +"%Y-%m-%dT%H:%M:%SZ")" --overwrite

# 3. Verify
kubectl get deploy -n podinfo podinfo -o jsonpath='replicas={.spec.replicas}{"\n"}'
```
