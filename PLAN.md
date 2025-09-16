# Vibe-Ensemble MCP — Actionable Engineering Plan

This plan guides implementation and hardening work without changing scope or architecture. It is structured for small, reviewable PRs with clear acceptance criteria. The dual output-processing paths are explicitly marked for investigation before any implementation work.

## Goals
- Align shipped behavior, docs, and versions across the codebase.
- Improve correctness and UX of MCP tools and queue-driven workers.
- Strengthen safety and operability (logging, configuration, observability).
- Add practical tests to validate core flows end-to-end.

## Non‑Goals (for now)
- New features beyond those already present in README/tools.
- Major refactors to the worker lifecycle unless approved.

## Assumptions
- SQLite remains the storage layer; migrations continue to be embedded via `include_str!`.
- Workers are spawned via Claude CLI; introduce configuration to reduce security risk.
- MCP protocol version used is `2024-11-05` across server, HTTP, and SSE.

---

## Workstream A — Version and Naming Consistency

Why: User‑visible inconsistency causes confusion and bugs.

Tasks
- A1. Align version strings across the codebase
  - Set a single source of truth (Cargo.toml `package.version`) and propagate to:
    - MCP `initialize` response `server_info.version` (src/mcp/server.rs)
    - SSE init notification version (src/sse.rs)
  - Add a small helper to pull version via `env!("CARGO_PKG_VERSION")` to avoid drift.
  - Acceptance: All endpoints return the same semantic version as Cargo.toml.

- A2. Tool naming alignment
  - `UpdateTicketStageTool` is exposed as `complete_ticket_stage` (definition). Confirm desired external name and ensure internal naming/comments match to avoid confusion in docs and prompts.
  - Acceptance: Tool name is consistent in list_tools, docs, prompts, and code comments.

- A3. Protocol version alignment
  - Ensure `MCP-Protocol-Version` checks, `initialize` response, `.mcp.json` and SSE init use `2024-11-05` consistently.
  - Acceptance: Grep confirms single protocol version string everywhere.

---

## Workstream B — Documentation and Prompts Sync

Why: Docs describe tools and flows that differ from current implementation.

Tasks
- B1. README updates
  - Remove/adjust references to non‑existent tools (e.g., `assign_task`, `get_queue_tasks`, explicit spawn/stop worker tools).
  - Clarify that workers are auto‑spawned by queues; Coordinator uses tickets and worker types only.
  - Strengthen security warning with configuration guidance (see Workstream C).
  - Acceptance: README tool lists map 1:1 to `list_tools`; workflow diagram notes auto‑queue behavior.

- B2. Prompts cleanup (MCP prompts + `.claude/commands/vibe-ensemble.md`)
  - Remove calls to non‑existent tools; emphasize minimal pipeline creation (start with `planning`) and worker type pre‑checks.
  - Ensure prompts reflect actual tool names and constraints (e.g., must create worker types before using stages).
  - Acceptance: Prompts are self‑consistent and runnable with current toolset.

- B3. Stage docs status update
  - `docs/TODO.md` shows “Not Started” for stages that now exist. Update status and validation checklists to reflect reality (server, DB layer, MCP tools, queues implemented; tests pending).
  - Acceptance: Stage statuses match implemented code and highlight remaining gaps.

---

## Workstream C — Security & Permissions Configuration

Why: Workers currently run with `--permission-mode bypassPermissions` which is risky.

Tasks
- C1. Add config surface (no behavior change yet)
  - Introduce configuration options (CLI + env) to control worker permission mode: `bypassPermissions | ask | restricted`.
  - Default to `ask` in development; document implications.
  - Acceptance: Config plumbed and validated; default visible in logs; behavior togglable without code changes.

- C2. Documentation
  - Expand README Security section with examples, risks, and recommended defaults.
  - Acceptance: Clear guidance and examples for safe operation, including local vs CI usage.

---

## Workstream D — Output Processing Architecture

Status: Requires deeper investigation and discussion before implementation.

Context
- There are two paths:
  - Legacy path: `QueueManager::output_processor_loop` handling `WorkerOutput`.
  - New domain‑based path: `workers/output/*` with `OutputProcessor` and domain types.

Risks/Questions
- Duplication can lead to divergence and bugs.
- Which path is canonical? Do we keep both for compatibility or migrate completely?
- Eventing and SSE integration points differ—unify strategy.

Action (Investigation Only)
- D1. Produce an ADR (Architecture Decision Record) comparing both paths:
  - Data flow diagrams, error handling, DB writes, SSE notifications, extensibility.
  - Migration plan (if any) with rollback strategy.
  - Performance and testability considerations.
  - Acceptance: ADR reviewed and approved before any coding changes.

Do NOT implement consolidation until ADR approval.

---

## Workstream E — Tests and Validation

Why: Core flows lack automated validation.

Tasks
- E1. Add a “mock worker” execution mode for CI
  - Feature flag to bypass spawning external `claude`; instead, inject deterministic `WorkerOutput`/completion events.
  - Acceptance: End‑to‑end tests run without external binaries or network access.

- E2. Integration tests
  - Start server on ephemeral port; exercise JSON‑RPC (`initialize`, `list_tools`, `create_project`, `create_worker_type`, `create_ticket`, `get_tickets_by_stage`, `add_ticket_comment`, `complete_ticket_stage`, `close_ticket`).
  - Validate DB effects and SSE emits (subscribe; assert at least one MCP message).
  - Acceptance: Tests green locally and in CI; no flakiness.

- E3. Migration tests
  - Boot with empty DB; verify schema_migrations populated through 004.
  - Boot from pre‑migration DB and ensure incremental upgrade path works.
  - Acceptance: All migrations apply idempotently; indices exist.

---

## Workstream F — Operability & Observability

Tasks
- F1. Logging polish
  - Ensure `env!("CARGO_PKG_VERSION")`, protocol version, host/port, DB path, respawn mode printed on startup.
  - Confirm file rotation and retention behavior; document logs path.
  - Acceptance: Key startup config visible; log files appear in `.vibe-ensemble-mcp/logs/`.

- F2. Health and readiness
  - `/health` already checks DB. Add readiness note in docs: DB connected, migrations applied, SSE broadcasting.
  - Acceptance: Operators know what “healthy” means and how to verify.

- F3. Graceful shutdown
  - Document behavior and expectations for in‑flight tasks; advise coordinator actions for on‑hold tickets.
  - Acceptance: Doc section added with recommended procedures.

---

## Workstream G — Developer Experience

Tasks
- G1. Config ergonomics
  - Document env var equivalents for CLI options (DB path, host, port, log level, no_respawn, permission mode).
  - Acceptance: One table in README; examples provided.

- G2. Worker templates
  - Ensure `.claude/worker-templates/*` align with tool names and JSON expectations (no non‑existent tools).
  - Acceptance: Templates are directly usable with `create_worker_type`.

---

## Workstream H — Release Hygiene

Tasks
- H1. Changelog and version bump
  - Introduce/maintain `CHANGELOG.md`; summarize changes and risks.
  - Align versions (see A1) and tag release.
  - Acceptance: Tag corresponds to Cargo.toml version and server reports same version.

- H2. Website refresh (optional)
  - Update website copy to match current features and security guidance.
  - Acceptance: Tool list and quick start are accurate.

---

## Execution Order (Recommended)
1) A1, A2, A3 — versions and names
2) B1, B2, B3 — docs and prompts alignment
3) C1, C2 — permission configuration surface + docs
4) E1, E2, E3 — tests and mock worker
5) F1, F2, F3 — operability polish
6) G1, G2 — DX improvements
7) D1 — output processing ADR (investigation only, prior to any code change)
8) H1, H2 — release hygiene

---

## PR Templates (per task)
- Summary: what changed and why.
- Scope: files touched; no unrelated changes.
- Tests: added/updated; manual steps if needed.
- Acceptance: checklist from this plan matched.
- Risk: note migration or behavior risks.

---

## Acceptance Checklists (Quick Copy‑Paste)

- Versions
  - [ ] `Cargo.toml` version = server `initialize` version = SSE init version
  - [ ] Single MCP protocol version across code and `.mcp.json`

- Docs/Prompts
  - [ ] Tools in README = `list_tools`
  - [ ] Prompts free of non‑existent tools and accurate sequences
  - [ ] TODO and stage docs reflect reality

- Security
  - [ ] Permission mode configurable and documented

- Tests
  - [ ] Mock worker path enabled in CI
  - [ ] E2E covers core ticket lifecycle
  - [ ] Migrations idempotent and validated

- Operability
  - [ ] Startup logs include key config
  - [ ] Health criteria documented
  - [ ] Shutdown behavior documented

- Output Processing (Investigation)
  - [ ] ADR written and approved before any consolidation work

---

## Open Questions for Discussion
- Which output processing path becomes canonical? How to migrate safely?
- What is the default and recommended worker permission mode for local dev vs CI?
- Do we want worker spawn/stop/list exposed as MCP tools, or keep implicit via queues?
- Should ticket stage names be constrained/validated against defined worker types at creation time?

---

This plan is intentionally surgical and review‑friendly. Use it to drive a series of small, auditable changes with strong validation and minimal risk.

