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

# Create namespace record and attach reference to a cluster
flux-resourceset-cli namespace create team-sandbox --cluster demo-cluster-01 \
  --label team=sandbox --annotation owner=platform

# Attach/detach an existing namespace record
flux-resourceset-cli namespace assign team-sandbox --cluster demo-cluster-01
flux-resourceset-cli namespace unassign team-sandbox --cluster demo-cluster-01
```

### Platform Component Operations

```bash
# List all catalog components
flux-resourceset-cli component list

# Get a specific component
flux-resourceset-cli component get cert-manager

# Create/ensure catalog component, then attach to cluster
flux-resourceset-cli component create cert-manager \
  --component-path core/cert-manager/1.14.0 \
  --component-version 1.14.0 \
  --oci-url oci://registry.example/platform-components \
  --oci-tag v1.0.0 \
  --cluster demo-cluster-01

# Attach/detach existing component references
flux-resourceset-cli component assign cert-manager --cluster demo-cluster-01
flux-resourceset-cli component unassign cert-manager --cluster demo-cluster-01

# Patch per-cluster component values
flux-resourceset-cli component patch podinfo --cluster demo-cluster-01 --set replicaCount=3
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
# 1. Create namespace + attach reference
flux-resourceset-cli namespace create team-sandbox --cluster demo-cluster-01 \
  --label team=sandbox --annotation owner=platform

# 2. Force reconcile
kubectl annotate resourcesetinputprovider namespaces -n flux-system \
  fluxcd.controlplane.io/requestedAt="$(date -u +"%Y-%m-%dT%H:%M:%SZ")" --overwrite
kubectl annotate resourceset namespaces -n flux-system \
  fluxcd.controlplane.io/requestedAt="$(date -u +"%Y-%m-%dT%H:%M:%SZ")" --overwrite

# 3. Wait and verify
kubectl get ns team-sandbox
```

### Patch a component and verify

```bash
# 1. Patch
flux-resourceset-cli demo patch-component demo-cluster-01 podinfo --set replicaCount=5

# 2. Refresh provider + resourceset
kubectl annotate resourcesetinputprovider platform-components -n flux-system \
  fluxcd.controlplane.io/requestedAt="$(date -u +"%Y-%m-%dT%H:%M:%SZ")" --overwrite
kubectl annotate resourceset platform-components -n flux-system \
  fluxcd.controlplane.io/requestedAt="$(date -u +"%Y-%m-%dT%H:%M:%SZ")" --overwrite

# 3. Trigger immediate Helm upgrade
flux reconcile helmrelease platform-podinfo -n flux-system --with-source

# 4. Verify
kubectl get deploy -n podinfo podinfo \
  -o jsonpath='replicas={.spec.replicas} color={.spec.template.spec.containers[0].env[?(@.name=="PODINFO_UI_COLOR")].value} message={.spec.template.spec.containers[0].env[?(@.name=="PODINFO_UI_MESSAGE")].value}{"\n"}'
```
