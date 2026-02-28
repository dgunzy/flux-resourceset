IMAGE_NAME := flux-resourceset
IMAGE_TAG := local
KIND_CLUSTER := flux-demo
FIRESTONE_RESOURCES := resources/cluster.yaml,resources/platform_component.yaml,resources/namespace.yaml,resources/rolebinding.yaml

.PHONY: build cli-build test lint fmt generate docker-build kind-create kind-setup kind-demo kind-delete demo cli-demo-port-forward cli-demo clean docs docs-serve docs-clean

build:
	cargo build

cli-build:
	cargo build --bin flux-resourceset-cli

test:
	cargo test

lint:
	cargo clippy -- -D warnings

fmt:
	cargo fmt

generate:
	env -u PYTHONPATH poetry install --no-interaction
	env -u PYTHONPATH poetry run firestone generate \
		-t 'Flux ResourceSet Internal API' \
		-d 'Internal API for Flux phone-home read-only and CRUD operations' \
		-v 2.0 \
		-r "$(FIRESTONE_RESOURCES)" \
		openapi \
		--version 3.0.3 \
		-O openapi/openapi.yaml
	rm -f src/generated/cli/*.rs
	env -u PYTHONPATH poetry run firestone generate \
		-t 'Flux ResourceSet Internal API CLI' \
		-d 'CLI for Flux ResourceSet CRUD API' \
		-v 2.0 \
		-r "$(FIRESTONE_RESOURCES)" \
		cli \
		--language rust \
		--pkg flux_resourceset \
		--client-pkg flux_resourceset \
		--as-modules \
		--output-dir src/generated/cli
	@tmpdir=$$(mktemp -d); \
	openapi-generator generate \
		-i openapi/openapi.yaml \
		-g rust \
		-o $$tmpdir \
		--skip-validate-spec \
		-c openapi/openapi-gen-config.json >/tmp/openapi-gen.log 2>&1; \
	rm -rf src/models src/apis; \
	cp -R $$tmpdir/src/models src/models; \
	cp -R $$tmpdir/src/apis src/apis; \
	for f in src/models/*.rs; do \
		if ! rg -q "models::" $$f; then sed -i '' '/use crate::models;/d' $$f; fi; \
	done; \
	rm -rf $$tmpdir
	cargo fmt

docker-build:
	docker build -t $(IMAGE_NAME):$(IMAGE_TAG) .

kind-create: docker-build
	kind create cluster --config hack/kind-config.yaml || true
	kind load docker-image $(IMAGE_NAME):$(IMAGE_TAG) --name $(KIND_CLUSTER)

kind-setup:
	kubectl create namespace flux-system || true
	# Install flux-operator (CRDs + controller) from upstream install manifest
	curl -fsSL https://github.com/controlplaneio-fluxcd/flux-operator/releases/latest/download/install.yaml | kubectl apply -f -
	kubectl rollout status deployment/flux-operator -n flux-system --timeout=5m
	# Apply base manifests
	kubectl apply -k k8s/base/
	kubectl wait --for=condition=Ready fluxinstance/flux -n flux-system --timeout=10m
	kubectl rollout status deployment/source-controller -n flux-system --timeout=5m
	kubectl rollout status deployment/kustomize-controller -n flux-system --timeout=5m
	# Create seed data ConfigMap
	kubectl create configmap flux-api-seed-data \
		--from-file=seed.json=data/seed.json \
		-n flux-system --dry-run=client -o yaml | kubectl apply -f -
	# Deploy the API
	kubectl apply -k k8s/demo/

kind-demo: kind-setup
	kubectl apply -k k8s/providers/
	kubectl apply -k k8s/resourcesets/
	@echo ""
	@echo "Demo deployed! Check status with:"
	@echo "  kubectl get pods -n flux-system"
	@echo "  kubectl get resourcesetinputproviders -n flux-system"
	@echo "  kubectl get resourcesets -n flux-system"

kind-delete:
	kind delete cluster --name $(KIND_CLUSTER)

demo: kind-create kind-demo

cli-demo-port-forward:
	@echo "Starting port-forward on 127.0.0.1:8080 -> flux-system/flux-api:8080"
	kubectl -n flux-system port-forward svc/flux-api 8080:8080

cli-demo:
	@set -eu; \
	TOKEN=$$(kubectl -n flux-system get secret internal-api-token -o jsonpath='{.data.token}' | base64 -d); \
	if ! curl -fsS http://127.0.0.1:8080/health >/dev/null; then \
		echo "No API detected on http://127.0.0.1:8080."; \
		echo "In another terminal run: make cli-demo-port-forward"; \
		exit 1; \
	fi; \
	cargo build --bin flux-resourceset-cli >/dev/null; \
	CLI=target/debug/flux-resourceset-cli; \
	if command -v jq >/dev/null 2>&1; then HAS_JQ=1; else HAS_JQ=0; fi; \
	echo "Using FLUX_API_URL=http://127.0.0.1:8080"; \
	echo ""; \
	echo "# Cluster list"; \
	if [ "$$HAS_JQ" -eq 1 ]; then \
		FLUX_API_URL=http://127.0.0.1:8080 FLUX_API_TOKEN="$$TOKEN" FLUX_API_WRITE_TOKEN="$$TOKEN" "$$CLI" cluster list | jq .; \
	else \
		FLUX_API_URL=http://127.0.0.1:8080 FLUX_API_TOKEN="$$TOKEN" FLUX_API_WRITE_TOKEN="$$TOKEN" "$$CLI" cluster list; \
	fi; \
	echo ""; \
	echo "# Namespace list (before)"; \
	if [ "$$HAS_JQ" -eq 1 ]; then \
		FLUX_API_URL=http://127.0.0.1:8080 FLUX_API_TOKEN="$$TOKEN" FLUX_API_WRITE_TOKEN="$$TOKEN" "$$CLI" namespace list | jq .; \
	else \
		FLUX_API_URL=http://127.0.0.1:8080 FLUX_API_TOKEN="$$TOKEN" FLUX_API_WRITE_TOKEN="$$TOKEN" "$$CLI" namespace list; \
	fi; \
	echo ""; \
	echo "# Add/update demo-runtime namespace via CLI"; \
	FLUX_API_URL=http://127.0.0.1:8080 \
	FLUX_API_TOKEN="$$TOKEN" \
	FLUX_API_WRITE_TOKEN="$$TOKEN" \
	"$$CLI" demo add-namespace demo-cluster-01 demo-runtime --label team=runtime --annotation owner=platform; \
	echo ""; \
	echo "# Force namespaces provider reconcile"; \
	REQUESTED_AT=$$(date -u +"%Y-%m-%dT%H:%M:%SZ"); \
	kubectl annotate resourcesetinputprovider namespaces -n flux-system fluxcd.controlplane.io/requestedAt="$$REQUESTED_AT" --overwrite >/dev/null; \
	kubectl annotate resourceset namespaces -n flux-system fluxcd.controlplane.io/requestedAt="$$REQUESTED_AT" --overwrite >/dev/null; \
	echo "Requested reconcile at $$REQUESTED_AT"; \
	echo ""; \
	echo "# Reconcile status"; \
	kubectl get resourcesetinputprovider namespaces -n flux-system; \
	kubectl get resourceset namespaces -n flux-system; \
	echo ""; \
	echo "# Wait for namespace demo-runtime to exist"; \
	for i in $$(seq 1 30); do \
		if kubectl get ns demo-runtime >/dev/null 2>&1; then \
			echo "demo-runtime created."; \
			break; \
		fi; \
		sleep 2; \
	done; \
	if ! kubectl get ns demo-runtime >/dev/null 2>&1; then \
		echo "Namespace demo-runtime was not created in time."; \
		kubectl get resourcesetinputproviders -n flux-system; \
		kubectl get resourcesets -n flux-system; \
		exit 1; \
	fi; \
	echo ""; \
	echo "# Flux namespaces output"; \
	if [ "$$HAS_JQ" -eq 1 ]; then \
		FLUX_API_URL=http://127.0.0.1:8080 FLUX_API_TOKEN="$$TOKEN" FLUX_API_WRITE_TOKEN="$$TOKEN" "$$CLI" demo flux-namespaces demo-cluster-01.k8s.example.com | jq .; \
	else \
		FLUX_API_URL=http://127.0.0.1:8080 FLUX_API_TOKEN="$$TOKEN" FLUX_API_WRITE_TOKEN="$$TOKEN" "$$CLI" demo flux-namespaces demo-cluster-01.k8s.example.com; \
	fi; \
	echo ""; \
	echo "# Namespace status"; \
	kubectl get ns demo-runtime

docs:
	@command -v mdbook >/dev/null 2>&1 || { echo "mdbook not found. Install with: cargo install mdbook"; exit 1; }
	@command -v mdbook-mermaid >/dev/null 2>&1 || { echo "mdbook-mermaid not found. Install with: cargo install mdbook-mermaid"; exit 1; }
	mdbook-mermaid install docs
	mdbook build docs
	@echo ""
	@echo "Docs built successfully. Open in your browser:"
	@echo "  file://$(CURDIR)/docs/book/index.html"

docs-serve:
	@command -v mdbook >/dev/null 2>&1 || { echo "mdbook not found. Install with: cargo install mdbook"; exit 1; }
	@command -v mdbook-mermaid >/dev/null 2>&1 || { echo "mdbook-mermaid not found. Install with: cargo install mdbook-mermaid"; exit 1; }
	mdbook-mermaid install docs
	@echo "Serving docs at http://localhost:3000"
	mdbook serve docs --open

docs-clean:
	rm -rf docs/book

clean: kind-delete docs-clean
