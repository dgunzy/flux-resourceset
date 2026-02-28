# AGENTS.md

## Mission

`flux-resourceset` is a **demo/reference implementation** of API-driven, phone-home GitOps for multi-cluster platforms.

The core concept to protect in every change:

1. Child clusters poll a central API via `ResourceSetInputProvider` (`type: ExternalService`)
2. API returns `{"inputs": [...]}` per cluster
3. `ResourceSet` templates render cluster-local resources
4. Flux reconciles those resources in each child cluster

This repo exists to make that pattern clear, credible, and easy to run.

## Scope and Priorities

- Optimize for:
  - Concept clarity
  - Correctness
  - Demo usability
  - Maintainability
- Avoid over-engineering and production-only complexity that obscures the teaching value

## Non-Negotiable Rules

- **Docs sync is required, not optional.**
- Any meaningful behavior/interface/config change must update:
  - `README.md`
  - relevant pages in `docs/src/` (mdBook)
  - examples/manifests if user-facing flow changes (`k8s/`, `Makefile` snippets)
- If code and docs diverge, treat that as a bug and fix it in the same change.

## Architecture Narrative to Preserve

- This project demonstrates why an API can manage multi-cluster desired state better than central push for many environments:
  - Pull-based resilience
  - Simpler network model (cluster outbound only)
  - Central policy/merge logic with per-cluster targeting
- Keep language and examples aligned with Flux Operator terms:
  - `ResourceSet`
  - `ResourceSetInputProvider`
  - `FluxInstance`
- Keep examples focused on understanding the control loop, not tool novelty.

## Firestone and Generated Code Contract

`resources/*.yaml` is the schema source of truth.

Generated artifacts:

- `openapi/openapi.yaml`
- `src/models/`
- `src/apis/`
- `src/generated/cli/`

Rules:

- Do not hand-edit generated files.
- For schema/API surface changes, update `resources/*.yaml` then run `make generate`.
- Keep handwritten logic in domain/merge/handlers/store/auth/config modules.

## Code Quality Expectations

- Favor straightforward, readable Rust over clever abstractions.
- Keep functions cohesive and side effects explicit.
- Preserve generated vs domain model separation.
- Maintain clear error handling boundaries (internal detail in logs, safe API responses).
- Add tests for behavioral changes (merge logic, auth, CRUD, API contract).

## Demo-First Product Expectations

- Assume local/demo operation is the first-class path.
- Keep setup and workflows digestible (`make demo`, CLI flow, quick verification commands).
- Prefer defaults that reduce friction while still reflecting realistic architecture.
- Keep payloads and examples small enough to understand at a glance.

## Change Checklist (run before finishing)

1. Code updated with minimal, clear design.
2. Tests added/updated for behavior changes.
3. Docs updated (`README.md` + relevant mdBook pages).
4. If schema changed: `make generate` run; generated outputs committed.
5. Validation run (as applicable):
   - `cargo fmt`
   - `cargo clippy -- -D warnings`
   - `cargo test`
   - `make build`
6. If docs tooling is present: `make docs`.

## Practical Commands

- `make build`
- `make test`
- `make lint`
- `make fmt`
- `make generate`
- `make demo`
- `make cli-demo`

## What Good Looks Like

A strong change in this repo:

- Makes the phone-home multi-cluster API concept easier to understand
- Improves reliability or correctness without hiding the core idea
- Keeps docs and code aligned
- Leaves the demo in a runnable, teachable state
