# Project Anatomy: AGI Workspace Audit Report

**Role:** Principal Rust Systems Auditor  
**Goal:** Map the Master Orchestrator and prepare for modular workspace expansion with a `crates/agi-core` library and `add-ons/` model.  
**Scope:** Structure analysis, dependency audit, knowledge base & skills, bare-metal compliance, decoupling strategy.  
**Constraint:** No code modifications—analysis and recommendations only.

---

## 1. Structure Analysis

### 1.1 Entry Point and Module Organization

| Component | Location | Role |
|-----------|----------|------|
| **Binary entry point** | `pagi-gateway/src/main.rs` | Single executable: Axum HTTP server, config load, wiring of memory/knowledge/orchestrator/skills, route handlers. |
| **Workspace root** | `Cargo.toml` (root) | Defines `[workspace]` with `members`: pagi-gateway, pagi-orchestrator, pagi-memory, pagi-knowledge, pagi-skills, pagi-shared. No `crates/` layout yet. |

**Flow from `main.rs`:**
1. **Tracing** – `tracing_subscriber` init from `RUST_LOG` (default `info`).
2. **Config** – `CoreConfig::load()` from `pagi-shared` (file + env; see §1.2).
3. **Paths** – `storage_path` from config → `memory_path = storage.join("pagi_vault")`, `knowledge_path = storage.join("pagi_knowledge")`.
4. **Memory** – `pagi_memory::MemoryManager::open_path(&memory_path)`.
5. **Knowledge** – `pagi_knowledge::KnowledgeStore::open_path(&knowledge_path)`.
6. **Skill registry** – `SkillRegistry::new()` then manual `registry.register(Arc::new(...))` for each of the 10 skills (LeadCapture, KnowledgeQuery, KnowledgeInsert, CommunityPulse, DraftResponse, ModelRouter, ResearchAudit, CommunityScraper, SalesCloser, KnowledgePruner).
7. **Blueprint** – `BlueprintRegistry::load_json_path(blueprint_path)` where `blueprint_path` is `PAGI_BLUEPRINT_PATH` or `"config/blueprint.json"`.
8. **Orchestrator** – `Orchestrator::with_blueprint(Arc::new(registry), blueprint)`.
9. **App** – `build_app(AppState { config, orchestrator, knowledge })` → Router with `/v1/status`, `/v1/execute`, `/v1/research/trace/:trace_id`, and optional static frontend.
10. **Serve** – `axum::serve` on `config.port`.

All core logic (goal dispatch, skill execution, blueprint planning) lives in **pagi-orchestrator**. The gateway is a thin HTTP shell that owns config, storage paths, and registration.

### 1.2 Short-term and Long-term Memory

| Layer | Implementation | Location | Behavior |
|-------|-----------------|----------|----------|
| **Short-term** | In-memory cache | `pagi-memory/src/lib.rs` | `DashMap<String, Vec<u8>>` keyed by `tenant_id:path`. Read path checks cache first; write path updates both cache and Sled. |
| **Long-term** | Sled DB | Same crate | Single `sled::Db` at configurable path (default `./data/pagi_vault`). Keys are path bytes; no tenant prefix in Sled (tenant is in cache key only). |

**Memory API:**
- `MemoryManager::new()` → `open_path(DEFAULT_VAULT_PATH)`.
- `MemoryManager::open_path(P)` → open/create Sled at `P`, new empty cache.
- `save_path(ctx, path, value)` → write to Sled and insert into cache (`cache_key(ctx, path)`).
- `get_path(ctx, path)` → cache lookup then Sled lookup; on Sled hit, backfill cache.

**Usage:** Only **LeadCapture** and **DraftResponse** use memory (lead history and context assembly). All paths are built in Rust (e.g. `lead_history/{tenant_id}/{lead_id}`); no external path scripts.

---

## 2. Dependency Audit

### 2.1 Workspace-Level Dependencies (`Cargo.toml` root)

| Crate | Purpose |
|-------|---------|
| tokio | Async runtime |
| serde / serde_json | Serialization |
| config | TOML + env config loading |
| axum | HTTP (gateway only) |
| sled | Persistent storage (memory + knowledge) |
| tracing | Logging |
| dashmap | In-memory concurrent map (memory cache) |
| reqwest | HTTP client (skills: future LLM, scrapers) |
| scraper | HTML parsing (CommunityScraper) |

### 2.2 Crate-Level Classification

| Crate | Type | Reason |
|-------|------|--------|
| **pagi-shared** | **Core** | Defines `Goal`, `TenantContext`, `CoreConfig`; used by orchestrator, gateway, memory, knowledge, skills. |
| **pagi-orchestrator** | **Core** | Master brain: `AgentSkill`, `SkillRegistry`, `Orchestrator`, `BlueprintRegistry`, `Plan`; goal dispatch and chaining. |
| **pagi-memory** | **Core** | Short- and long-term memory; required for lead capture and context assembly. |
| **pagi-knowledge** | **Core** | 8-slot KB store and `KnowledgeSource` trait; required for query/insert and all KB-using skills. |
| **pagi-skills** | **Add-on / optional** | Concrete skills implementing `AgentSkill`; depend on orchestrator (trait), memory, knowledge, shared. Could live in `add-ons/` or remain in workspace as first-party add-ons. |
| **pagi-gateway** | **Utility / host** | Entry binary: HTTP, config load, registration, static UI. Depends on all of the above. |

**Utility (logging / config / transport):**
- **tracing** – logging (utility).
- **config** – config load in pagi-shared (utility for host; core types remain in shared).
- **axum**, **tower-http** – gateway only (utility for the host binary).

**Summary:** Core logic and data types are in **pagi-shared**, **pagi-orchestrator**, **pagi-memory**, **pagi-knowledge**. The gateway is the only binary and is a utility host that wires core + skills.

---

## 3. Knowledge Base & Skills

### 3.1 The 8 Knowledge Bases

Implemented as **eight Sled trees** inside a single `KnowledgeStore` DB:

| Slot | Tree name (internal) | Purpose (from config labels) |
|------|----------------------|------------------------------|
| 1 | kb1_marketing | Brand Voice, marketing |
| 2 | kb2_sales | Sales, closing strategy |
| 3 | kb3_finance | Finance |
| 4 | kb4_operations | Operations |
| 5 | kb5_community | Community pulse, local events |
| 6 | kb6_products | Products |
| 7 | kb7_policies | Policies |
| 8 | kb8_custom | Custom / internal (e.g. research traces) |

**Code layout:**
- **store** – `KnowledgeStore`: single `sled::Db`, `open_path(P)`, `get(slot_id, key)`, `insert`, `remove`, `scan_keys(slot_id)`. Tree name from constant array `TREE_NAMES[slot_id - 1]`.
- **kb1..kb8** – Thin wrappers: each `KbN(Arc<KnowledgeStore>)` implements `KnowledgeSource` (slot_id, name, query). No extra logic; they delegate to `store.get(slot_id, query_key)`.

**Initialization:** The 8 KBs are **not** instantiated as separate objects at startup. The gateway opens **one** `KnowledgeStore` at `knowledge_path` and passes `Arc<KnowledgeStore>` to the skills that need it. The slot semantics (1–8) are in the store’s tree names and in the goals (e.g. `QueryKnowledge { slot_id, query }`, `UpdateKnowledgeSlot { slot_id, ... }`). So “8 Knowledge Bases” = one store with 8 trees; the Kb1..Kb8 types exist for a trait-based view but are **not** used in the gateway or orchestrator today—only `KnowledgeStore` is.

### 3.2 Skills-Type Solution (Trait + Registry)

- **Trait:** `pagi_orchestrator::AgentSkill`: `fn name(&self) -> &str` and `async fn execute(ctx, payload) -> Result<Value, Box<Error>>`.
- **Registry:** `SkillRegistry`: `Vec<Arc<dyn AgentSkill>>`, `register()`, `get(name)`, `skill_names()`.
- **Orchestrator** holds `Arc<SkillRegistry>` and `Arc<BlueprintRegistry>`. It dispatches goals by matching on `Goal` and either calling a single skill (e.g. `ExecuteSkill` → `registry.get(name)`), or running a chain (e.g. `AutonomousGoal` → blueprint plan → for each step `registry.get(step)` and `chain_payload(previous_skill, next_skill, previous_result, payload)`).

**Concrete skills (in pagi-skills):**
- LeadCapture (memory)
- KnowledgeQuery, KnowledgeInsert, KnowledgePruner (knowledge)
- CommunityPulse, CommunityScraper (knowledge, mainly KB-5)
- DraftResponse (memory + knowledge)
- ModelRouter (env: PAGI_LLM_MODE, PAGI_LLM_API_URL, PAGI_LLM_API_KEY)
- ResearchAudit (knowledge, KB-8 traces)
- SalesCloser (knowledge)

**Initialization:** Explicit in **pagi-gateway** `main.rs`: construct each skill with its dependencies (e.g. `LeadCapture::new(Arc::clone(&memory))`), then `registry.register(Arc::new(...))`. Blueprint is loaded from JSON; orchestrator is built with registry + blueprint. No plugin/dynamic loading—all skills are compiled into the gateway binary.

---

## 4. Bare-Metal Compliance

### 4.1 No Docker / Containers

- **Dockerfiles:** None found (search for `Dockerfile*` returned 0 files).
- **Container-specific env loaders:** None. Config is file + `PAGI__*` env vars via `config` crate; no `.env` file loading or `load_dotenv`-style code.

### 4.2 Non-Rust Scripts

- No `*.sh`, `*.bat`, `*.ps1` in the repo used for startup or env loading. Only Rust code and config (TOML/JSON) drive behavior.

### 4.3 Paths: Filesystem-Relative

- **Config path:** `CoreConfig::load()` uses `std::env::var("PAGI_CONFIG").unwrap_or_else(|_| "config/gateway".to_string())` then `Path::new(&config_path)`. So paths are relative to process CWD or explicit env path—no hardcoded container paths.
- **Storage:** `storage_path` from config (default `"./data"`); memory and knowledge paths are `storage_path.join("pagi_vault")` and `storage_path.join("pagi_knowledge")`. All relative to CWD when not overridden.
- **Blueprint:** `PAGI_BLUEPRINT_PATH` or `"config/blueprint.json"`—relative.
- **Default paths in crates:** `pagi-memory`: `./data/pagi_vault`; `pagi-knowledge` store: `./data/pagi_knowledge`. Only used when `open_path` is not called with an explicit path; gateway always passes paths derived from config.
- **Frontend:** `frontend_root_dir()` uses `std::env::current_dir().join("pagi-frontend")` first, then falls back to `env!("CARGO_MANIFEST_DIR").join("..").join("pagi-frontend")`. So CWD-relative first, then workspace-relative at build time. No container assumptions.

**Verdict:** The project is bare-metal and filesystem-relative. No Dockerfiles, no container env loaders, no non-Rust startup scripts. Optional env vars (`PAGI_CONFIG`, `PAGI_BLUEPRINT_PATH`, `PAGI_LLM_*`) override paths/mode without introducing container coupling.

---

## 5. Decoupling Strategy: `crates/agi-core` and `add-ons/`

### 5.1 What Belongs in `crates/agi-core`

**Proposed library crate:** `agi-core` (or `pagi-core`) should contain everything needed to run the “brain” and storage without HTTP or concrete skills:

| Current crate | Content to move / re-export from agi-core |
|---------------|--------------------------------------------|
| **pagi-shared** | Move entirely into `agi-core` (or keep as internal sub-crate under `crates/agi-core` and re-export). Types: `TenantContext`, `Goal`, `CoreConfig`. |
| **pagi-orchestrator** | Move entirely: `AgentSkill`, `SkillRegistry`, `Orchestrator`, `BlueprintRegistry`, `Plan`, `blueprint`, `planner`, and dispatch/chain logic. |
| **pagi-memory** | Move entirely: `MemoryManager`, short-term cache + Sled long-term. |
| **pagi-knowledge** | Move entirely: `KnowledgeStore`, `KnowledgeSource`, Kb1..Kb8, store (8 trees). |

**Resulting `agi-core` surface:**
- Open memory and knowledge by path (no hardcoded defaults in core; caller passes paths).
- Build orchestrator with a `SkillRegistry` and `BlueprintRegistry` (no knowledge of HTTP or specific skills).
- Dispatch `Goal` with `TenantContext` and get `Result<Value, Error>`.

So: **core = shared + orchestrator + memory + knowledge** as one library, with no dependency on axum, pagi-skills, or pagi-gateway.

### 5.2 What Stays in the Workspace (Outside Core)

| Item | Role |
|------|------|
| **pagi-gateway** | Binary: load config, open storage via paths from config, build `SkillRegistry` and register skills, load blueprint, build `Orchestrator`, run Axum. Depends on `agi-core` and on whatever provides skills (e.g. add-ons). |
| **pagi-skills** | Becomes an **add-on** (or first add-on): implements `AgentSkill` using `agi-core` (memory, knowledge, orchestrator trait). Depends only on `agi-core` (and optionally shared types from core). |
| **pagi-frontend** | Static UI; served by gateway. No change. |
| **config/** | TOML/JSON config and blueprint; path source for gateway. Can stay at repo root or move under a “host” area. |

### 5.3 How `add-ons/` Should Interface With Core

- **Trait:** Add-ons depend on `agi-core` and implement `AgentSkill` (name + execute).
- **Registration:** The **host** (e.g. pagi-gateway) constructs one `SkillRegistry`, opens `MemoryManager` and `KnowledgeStore` (paths from config), then instantiates each add-on with the capabilities it needs (e.g. `Arc<MemoryManager>`, `Arc<KnowledgeStore>`) and calls `registry.register(Arc::new(addon_skill))`. So add-ons do not register themselves; the host wires them.
- **Discovery:** Optional: add-ons could expose a single function (e.g. `pub fn register(registry: &mut SkillRegistry, memory: Arc<MemoryManager>, knowledge: Arc<KnowledgeStore>)`) so the host only calls `pagi_skills_addon::register(&mut registry, memory, knowledge)`. Today’s explicit list in `main.rs` is equivalent—just a different place to enumerate skills.
- **Blueprint:** Intent → skill names remain in the blueprint (JSON). Add-ons must use the same skill names as in the blueprint for `AutonomousGoal` to work. No change to blueprint format.

**File-level move list (conceptual):**

- Into `crates/agi-core/src/` (or submodules):
  - From pagi-shared: `lib.rs` (TenantContext, Goal, CoreConfig).
  - From pagi-orchestrator: `lib.rs`, `blueprint.rs`, `planner.rs`.
  - From pagi-memory: `lib.rs`.
  - From pagi-knowledge: `lib.rs`, `store.rs`, `kb1.rs`..`kb8.rs`.
- `agi-core`’s public API: re-export what gateway and add-ons need: e.g. `CoreConfig`, `Goal`, `TenantContext`, `MemoryManager`, `KnowledgeStore`, `KnowledgeSource`, `AgentSkill`, `SkillRegistry`, `Orchestrator`, `BlueprintRegistry`, `Plan`.

### 5.4 Dependency Direction After Split

```
pagi-gateway (binary)  →  agi-core, add-ons (e.g. pagi-skills)
add-ons (pagi-skills)  →  agi-core only
agi-core               →  no pagi-* crates; only workspace deps (tokio, serde, sled, dashmap, config, etc.)
```

This keeps core free of HTTP and concrete skill implementations and makes it safe to add more add-ons (or alternate hosts) without touching core.

---

## 6. Summary Table

| Aspect | Finding |
|--------|---------|
| **Entry point** | `pagi-gateway/src/main.rs` – single binary. |
| **Master orchestrator** | `pagi-orchestrator`: `Orchestrator` + `SkillRegistry` + `BlueprintRegistry`; goal dispatch and chaining in `dispatch()`. |
| **Short-term memory** | `pagi-memory`: DashMap cache keyed by tenant+path. |
| **Long-term memory** | `pagi-memory`: single Sled DB at configurable path. |
| **Knowledge** | One `KnowledgeStore` (pagi-knowledge) with 8 Sled trees (kb1–kb8); Kb1..Kb8 are thin trait impls, not used in gateway. |
| **Skills** | Trait `AgentSkill` in orchestrator; 10 concrete skills in pagi-skills; registered explicitly in gateway `main`. |
| **Blueprint** | JSON: intent → list of skill names; loaded from file in gateway, used by orchestrator for `AutonomousGoal`. |
| **Core crates** | pagi-shared, pagi-orchestrator, pagi-memory, pagi-knowledge. |
| **Utility crates** | pagi-gateway (host), tracing/config/axum (support). |
| **Bare-metal** | No Dockerfiles, no container env, no non-Rust scripts; paths are filesystem-relative (CWD or config/env). |
| **Decoupling** | Move shared + orchestrator + memory + knowledge into `crates/agi-core`; gateway + add-ons (e.g. pagi-skills) depend on core; add-ons implement `AgentSkill` and are registered by the host. |

---

*End of Project Anatomy report. No code was modified.*
