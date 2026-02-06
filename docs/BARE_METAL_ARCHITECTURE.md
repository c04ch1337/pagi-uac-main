# PAGI Ecosystem: Bare-Metal Architecture Rules

This document is the single source of truth for layout, naming, and constraints. All crates and add-ons must comply.

---

## 1. Naming & Prefixes

- **All internal crates and add-ons MUST use the `pagi-` prefix.**
- Examples: `pagi-core`, `pagi-skills`, `pagi-gateway`, `pagi-companion-ui`, `pagi-offsec-ui`, `pagi-personal-ui`.

---

## 2. Workspace Constraints (NO DOCKER)

- **Shared Brain:** Any logic for memory, orchestration, or knowledge bases (KBs) belongs in **`crates/pagi-core`**. No other crates under `crates/` should contain core brain logic.
- **Add-ons:** All UIs live in **`add-ons/pagi-[name]-ui`**. **`add-ons/pagi-studio-ui`** is the **Primary** (Developer's Cockpit, reference). Other add-ons (Companion, OffSec, Personal) are specialized. Each add-on may depend on `pagi-core` only; no add-on may depend on another add-on’s UI.
- **Environment:** Everything is **Bare-Metal**. No Docker, no container-specific env loaders. Paths must be relative to the executable using `std::env::current_dir()` or local config files (e.g. `config/gateway.toml`, `PAGI_CONFIG`, `PAGI_BLUEPRINT_PATH`).

---

## 3. Frontend Separation

- **`pagi-core` must remain UI-agnostic.** No GUI crates (e.g. Slint, Ratatui, iced) are allowed in **`crates/`**.
- Each **pagi-add-on** is responsible for its own UI implementation and thread management. The gateway may serve static assets (e.g. `pagi-frontend/`) but core logic never depends on a specific UI stack.

---

## 4. Skills & Knowledge

- **8 KB trees (Sled)** are managed by the **knowledge** module inside **`pagi-core`** (the former pagi-knowledge component). Slot IDs 1–8 map to trees `kb1_marketing` … `kb8_custom`.
- **New Skills** must be implemented in **`pagi-skills`** or in a dedicated add-on folder, and **registered via the `SkillRegistry`** (provided by `pagi-core`). The gateway (or host binary) wires skills at startup; add-ons may contribute skills by implementing the `AgentSkill` trait and registering with the host’s registry.

---

## Workspace Layout Summary

| Location              | Purpose                                      |
|-----------------------|----------------------------------------------|
| `crates/pagi-core`    | Shared types, orchestrator, memory, 8-slot KB |
| `add-ons/pagi-studio-ui` | **Primary** — Developer's Cockpit, reference frontend |
| `add-ons/pagi-*-ui`   | Other UI add-ons (Companion, OffSec, Personal) |
| `pagi-gateway`        | Single binary entry point; HTTP, config, skill registration |
| `pagi-skills`         | Default skill implementations (LeadCapture, KnowledgeQuery, etc.) |

See **[FRONTEND_DESIGN_SYSTEM.md](FRONTEND_DESIGN_SYSTEM.md)** for add-on UI stacks (Slint, Ratatui, Egui) and the rule that all frontends call `pagi_core::Orchestrator::dispatch()` for AGI reasoning.
