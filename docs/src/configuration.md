# Configuration & Deployment

## Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `API_MODE` | no | `read-only` | Runtime mode: `read-only` or `crud` |
| `STORE_BACKEND` | no | `sqlite` | Data backend: `sqlite` or `memory` |
| `DATABASE_URL` | no | `sqlite://data/flux-resourceset.db?mode=rwc` | SQLite DSN when `STORE_BACKEND=sqlite` |
| `AUTH_TOKEN` | yes | — | Bearer token for read routes |
| `CRUD_AUTH_TOKEN` | no | `AUTH_TOKEN` | Bearer token for write routes in CRUD mode |
| `SEED_FILE` | no | `data/seed.json` | Seed data file loaded at startup |
| `OPENAPI_FILE` | no | `openapi/openapi.yaml` | OpenAPI document served at `/openapi.yaml` |
| `LISTEN_ADDR` | no | `0.0.0.0:8080` | Bind address |
| `RUST_LOG` | no | unset | Tracing filter directive |

## Runtime Modes

### read-only

The default mode. Serves only Flux read endpoints (`/api/v2/flux/...`) and service endpoints (`/health`, `/ready`, `/openapi.yaml`). Designed for high-concurrency polling from many clusters.

```bash
API_MODE=read-only AUTH_TOKEN=my-token cargo run
```

### crud

Full CRUD mode. Includes all read endpoints plus REST endpoints for clusters, platform_components, namespaces, and rolebindings. Used by operators and CI/CD pipelines.

```bash
API_MODE=crud AUTH_TOKEN=read-token CRUD_AUTH_TOKEN=write-token cargo run
```

## Production Deployment

### Kubernetes Deployment (read-only)

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: flux-api-read
  namespace: flux-system
spec:
  replicas: 2
  selector:
    matchLabels:
      app: flux-api-read
  template:
    metadata:
      labels:
        app: flux-api-read
    spec:
      containers:
        - name: flux-api
          image: flux-resourceset:latest
          ports:
            - containerPort: 8080
          env:
            - name: API_MODE
              value: "read-only"
            - name: STORE_BACKEND
              value: "sqlite"
            - name: DATABASE_URL
              value: "sqlite:///var/lib/flux-resourceset/flux-resourceset.db?mode=rwc"
            - name: SEED_FILE
              value: "/seed/seed.json"
            - name: AUTH_TOKEN
              valueFrom:
                secretKeyRef:
                  name: internal-api-token
                  key: token
            - name: RUST_LOG
              value: "info"
          resources:
            requests:
              cpu: 50m
              memory: 32Mi
            limits:
              cpu: 200m
              memory: 64Mi
          livenessProbe:
            httpGet:
              path: /health
              port: 8080
            initialDelaySeconds: 2
            periodSeconds: 10
          readinessProbe:
            httpGet:
              path: /health
              port: 8080
            initialDelaySeconds: 2
            periodSeconds: 5
```

Resource requests are deliberately small — Rust's efficiency means this service uses minimal resources. Run 2+ replicas for high availability, not for throughput.

### Performance Characteristics

Each request does a data store lookup and a merge. Expected latency is sub-millisecond for the in-memory backend and typically single-digit milliseconds for SQLite on local SSD.

| Clusters | Poll Interval | Requests/sec |
|----------|--------------|-------------|
| 50 | 5 min | 0.17 |
| 200 | 5 min | 0.67 |
| 1,000 | 5 min | 3.3 |
| 5,000 | 5 min | 16.7 |

Even at 5,000 clusters with three resource types each, the load is ~50 req/sec — trivial for a Rust/axum service.

## Build Commands

```bash
cargo build                    # Build API + CLI
cargo build --bin flux-resourceset-cli  # Build CLI only
cargo test                     # Run all tests
cargo clippy -- -D warnings    # Lint
cargo fmt                      # Format
```

## Docker

```bash
make docker-build              # Build container image
```

## Code Generation

The project uses Firestone for schema-driven code generation:

```bash
make generate
```

This regenerates:
- `openapi/openapi.yaml` — OpenAPI 3.0 spec
- `src/models/` — Rust model structs
- `src/apis/` — Rust API client modules
- `src/generated/cli/` — CLI command modules
