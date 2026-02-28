# Frequently Asked Questions

## Architecture & Design Decisions

### Why an API instead of direct Kubernetes API access?

A common reaction is: "Why not just give operators `kubectl` access or build tooling that talks directly to the Kubernetes API on each cluster?"

The answer comes down to **control, safety, and scale**:

| Concern | Direct Kubernetes API | Purpose-built API (this) |
|---------|----------------------|--------------------------|
| **Blast radius** | One bad `kubectl apply` can break a cluster. Operators need kubeconfig access to every cluster. | All changes flow through a single API with validation. No direct cluster access needed for platform operations. |
| **Business logic** | The Kubernetes API has no concept of "platform components," "environment tiers," or "component catalogs." You build that logic into scripts. | The API encodes your organization's domain model. Merge logic, catalog defaults, environment resolution, and patching rules are built in. |
| **Audit trail** | Kubernetes audit logs are per-cluster and verbose. Correlating "who changed what across 200 clusters" is painful. | One API, one audit log. Every mutation is traceable to a user, timestamp, and change payload. |
| **Integration** | Integrating CI/CD, chatops, ticketing, or approval workflows with raw Kubernetes APIs across many clusters requires custom glue per cluster. | One REST API to integrate with. Webhooks, CI pipelines, Slack bots, and approval systems all talk to one endpoint. |
| **Credential management** | Operators (or CI) need kubeconfigs for every cluster. Rotating credentials means touching every cluster. | Operators need one API token. Clusters hold one read token. Token rotation is centralized. |
| **Consistency** | Without enforcement, two operators can configure the same component differently on two clusters. Scripts drift. | The catalog + merge model guarantees consistent computed state. Per-cluster differences are explicit and auditable. |
| **Rollback** | Rolling back a `kubectl apply` requires knowing exactly what was applied and in what order. | Revert the API data. Next poll cycle, Flux reconciles back. |

**In short:** The Kubernetes API is a powerful infrastructure primitive, but it is not a platform management API. This service adds the domain logic, guardrails, and integration surface that enterprise operations require.

### Is this actually GitOps?

Yes — with a nuance. This is a **GitOps-based model** that adds an API-driven data layer.

The GitOps principles are preserved:

- **Declarative** — desired state is declared in structured data (API) and templates (Git)
- **Versioned and immutable** — templates are version-controlled in Git. API data changes are auditable and reversible.
- **Pulled automatically** — clusters pull their state; no manual push required
- **Continuously reconciled** — Flux detects and corrects drift automatically

What the API adds:

- **Dynamic data** — instead of static YAML files per cluster, the API computes each cluster's state from catalog + overrides
- **Operational velocity** — data changes (scaling, patching, enabling/disabling) do not require Git PRs
- **Business logic** — merge rules, catalog defaults, and environment resolution happen in the API, not in Git overlays

The templates that govern *how* resources are deployed still live in Git and go through standard review. The API controls *what* is deployed *where* — the operational data plane.

### Why not ArgoCD ApplicationSets?

ArgoCD ApplicationSets solve a similar problem (managing resources across many clusters) but take a fundamentally different approach:

| Aspect | ArgoCD ApplicationSets | This architecture |
|--------|----------------------|-------------------|
| **Model** | Push from management cluster | Pull from each cluster |
| **Management cluster dependency** | Required — ArgoCD must maintain connections to all clusters | Not required for platform management — clusters are autonomous |
| **Failure mode** | Management cluster down = no reconciliation anywhere | API down = clusters keep running, just cannot get updates |
| **Kubeconfig management** | ArgoCD needs kubeconfigs for every target cluster | Each cluster holds one API bearer token |
| **Network direction** | Management cluster → target clusters (requires inbound access to clusters) | Target clusters → API (outbound only) |
| **Data source** | Git repos with generators (list, cluster, git, matrix) | API with merge logic and dynamic catalog |
| **Per-cluster overrides** | Generators + overlays (can get complex) | First-class `patches` object in the API |

Both are valid approaches. ApplicationSets work well when you have a stable management cluster with reliable connectivity to all targets. The phone-home model works better when clusters are distributed, network connectivity is unreliable, or you need clusters to be autonomous.

### Does this work on-premises?

Yes. The architecture is **infrastructure-agnostic**. It has no dependency on any specific cloud provider, VM provisioner, or Kubernetes distribution.

| Environment | Requirements |
|-------------|-------------|
| **On-prem bare metal** | Kubernetes cluster with Flux Operator installed. Outbound HTTPS to the API. |
| **On-prem VMs** | Same — any hypervisor (VMware, KVM, Hyper-V). |
| **Public cloud (EKS, AKS, GKE)** | Deploy Flux Operator as a Helm chart or add-on. |
| **Edge / remote sites** | Lightweight K8s (k3s, k0s, MicroK8s). Can work over VPN or direct internet. |
| **Air-gapped** | Possible with a local API mirror and OCI registry mirror inside the air gap. |
| **Hybrid** | Mix any of the above. Every cluster phones home to the same API. |

The provisioning tooling is completely decoupled. Whether you use Terraform, Cluster API, Crossplane, Rancher, manual scripts, or your own management cluster — once Flux is running and the `cluster-identity` ConfigMap exists, the phone-home loop works.

### Why separate read-only and CRUD modes?

The two modes serve fundamentally different access patterns:

| Mode | Consumers | Pattern | Scaling |
|------|-----------|---------|---------|
| `read-only` | Hundreds/thousands of clusters polling | High concurrency, small payloads, predictable load | Multi-replica, horizontal scaling |
| `crud` | Operators, CLI, CI/CD pipelines | Low concurrency, larger payloads, bursty | Single replica or small deployment |

Separating them gives you:

- **Independent scaling** — read replicas scale with fleet size; CRUD does not need to
- **Security boundary** — read-only instances never accept writes; separate tokens for each
- **Blast radius** — a CRUD deployment issue does not affect cluster polling
- **Simpler operations** — read-only instances are stateless and disposable

## Operational Questions

### What happens if the API goes down?

Clusters **keep running**. They continue reconciling from their last-known state. Existing HelmReleases, Namespaces, and ClusterRoleBindings all remain in place and healthy.

What stops working:

- New configuration changes are not picked up until the API recovers
- The ResourceSetInputProvider status shows not-ready
- Alerts should fire based on provider status conditions

This is a key advantage over push-based models — API downtime is an inconvenience, not an outage.

### How do I roll back a bad change?

1. **Revert the API data** — update the cluster document or catalog entry back to the previous state
2. **Wait for next poll** — or force an immediate reconcile with `kubectl annotate`
3. **Flux reconciles** — the ResourceSet re-renders with the reverted data, and Flux applies the diff

For template changes (in Git), use standard Git revert workflows. Flux picks up the reverted template on next reconcile.

### How do I handle secrets?

The patches object is for **non-sensitive configuration only** (replica counts, feature flags, resource limits). For secrets:

- Use the [External Secrets Operator](https://external-secrets.io/) to sync secrets from a vault (HashiCorp Vault, AWS Secrets Manager, Azure Key Vault, etc.)
- Reference Kubernetes Secrets in HelmRelease `valuesFrom` instead of ConfigMaps
- Add an `external-secrets` resource type to the API to manage ESO `ExternalSecret` resources via the same phone-home pattern

### Can I use this with existing Flux installations?

Yes. The ResourceSetInputProvider and ResourceSet are standard Flux Operator CRDs. They coexist with existing GitRepositories, HelmRepositories, Kustomizations, and HelmReleases.

You can adopt incrementally:

1. Install the Flux Operator alongside existing Flux controllers
2. Deploy providers and ResourceSets for one resource type (e.g., namespaces)
3. Migrate additional resource types as confidence grows
4. Existing Git-based Flux resources continue working unchanged

### How does this compare to Helm value files per cluster?

| Aspect | Helm values per cluster | API-driven patching |
|--------|------------------------|---------------------|
| **Storage** | YAML files in Git (one per cluster, or overlays) | Structured data in the API |
| **Updating 100 clusters** | 100 file edits + PR | Batch API call |
| **Per-cluster customization** | Overlay hierarchy (can get deeply nested) | Flat `patches` object per cluster per component |
| **Dynamic values** | Requires scripted Git commits | API call → next poll → reconciled |
| **Review requirement** | Git PR for every change (even scaling) | API auth for data changes; Git PR for template changes |
| **Merge conflicts** | Possible with concurrent PRs | Not possible — API handles concurrency |

### Can I extend this beyond platform components?

Yes. The architecture is designed for it. Any Kubernetes resource type can be managed this way. See the [Extending](./extending.md) chapter for a step-by-step walkthrough.

Ideas that organizations have considered:

- Network policies
- Resource quotas and limit ranges
- External secrets
- Ingress routes and TLS certificates
- Custom CRDs specific to the organization
- Monitoring and alerting configurations (PrometheusRule, ServiceMonitor)

Each follows the same pattern: schema, endpoint, provider, template.

## Performance & Scale

### How many clusters can this support?

The API is stateless and the per-request cost is minimal (one data store read + one merge). Rough numbers:

| Clusters | Resource Types | Poll Interval | Requests/sec |
|----------|---------------|---------------|-------------|
| 100 | 3 | 5 min | 1 |
| 500 | 3 | 5 min | 5 |
| 1,000 | 3 | 5 min | 10 |
| 5,000 | 3 | 5 min | 50 |
| 10,000 | 5 | 5 min | 167 |

Even at 10,000 clusters with 5 resource types, the load is ~167 req/sec — well within the capacity of a small API deployment. Add read replicas for HA, not for throughput.

### What is the latency from API change to cluster reconciliation?

It depends on the poll interval configured on the ResourceSetInputProvider. The default is 5 minutes. For faster feedback:

- Set `fluxcd.controlplane.io/reconcileEvery: "30s"` on the provider (the demo uses this)
- Force immediate reconciliation by annotating the provider with `fluxcd.controlplane.io/requestedAt`
- In practice, 5-minute intervals are fine for production — platform component changes are not latency-sensitive

### Does every cluster get the full catalog?

No. Each cluster only receives the components, namespaces, and rolebindings assigned to it in the cluster document. The API computes a cluster-specific response — a cluster with 5 components gets 5 inputs, not the entire catalog.
