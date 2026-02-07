# Frontend ↔ Backend Integration (Gateway/Bridge, KB, Memory, Prompts)

This document is the **version-controlled integration runbook** for wiring any Frontend UI into the PAGI backend.

Scope:

* **Bridge/Gateway integration** (HTTP API surface, streaming, logs)
* **Orchestrator integration** (goals + skills execution)
* **Knowledge Base (KB) integration** (8-slot ontology, labels, query/insert)
* **Memory integration** (short-term “vault” + conversational/episodic storage)
* **Prompt inventory** (system prompts and prompt-injection contexts used by the backend)
* **Copy/paste “Integration Prompts”** you can use with an LLM or as internal SOP prompts when standing up a new Frontend

---

## 0) Step-by-step integration (end-to-end)

Use this ordered checklist to tie **backend → gateway → engine → frontend** from scratch.

### Phase 1: Backend and gateway

1. **Pre-flight**  
   Run `cargo run -p pagi-gateway -- --verify` from the workspace root. This checks port 8001 and that no Sled DB locks (e.g. in `data/pagi_vault/`, `data/pagi_knowledge/`) are held. Fix any port or lock issues before starting.

2. **Config**  
   Confirm [`config/gateway.toml`](config/gateway.toml): `port` (default 8001), `storage_path`, `llm_mode`, `frontend_enabled`, and `[slot_labels]` for the 8 KB slots.

3. **Start gateway**  
   From workspace root: `cargo run -p pagi-gateway`. On first run, bootstraps (core identity KB-1, core skills KB-5, ethos KB-6) and optional workspace scan (Oikos) run automatically.

4. **Verify backend**  
   - `GET http://127.0.0.1:8001/v1/status` → app name, port, `llm_mode`, `slot_labels`.  
   - `GET http://127.0.0.1:8001/api/v1/health` → `{"status":"ok"}`.  
   - `GET http://127.0.0.1:8001/api/v1/kb-status` → status of all 8 Knowledge Bases.

### Phase 2: Choose frontend and wire URLs

**Option A – Drop-in UI (same origin)**  
- Gateway serves `pagi-frontend` when `frontend_enabled = true`.  
- Open `http://127.0.0.1:8001/` (index) or `http://127.0.0.1:8001/ui/`.  
- Frontend calls **same origin**: e.g. `POST /v1/execute` for autonomous goals (see [`pagi-frontend/app.js`](pagi-frontend/app.js)).

**Option B – Studio UI (separate dev server)**  
- In one terminal: keep `cargo run -p pagi-gateway` (port 8001).  
- In another: `cd add-ons/pagi-studio-ui/assets/studio-interface && npm run dev` (Vite, port 3001).  
- Open `http://127.0.0.1:3001`.  
- In Studio Settings, set **API URL** to `http://127.0.0.1:8001/api/v1/chat`.  
- Log terminal in Studio uses `http://127.0.0.1:8001/api/v1/logs` (SSE).  
- KB status uses same origin derived from API URL: `{origin}/api/v1/kb-status`.

### Phase 3: Verify end-to-end

5. **CORS**  
   Gateway allows origins on ports 3001–3099 (Frontend) and 8001–8099 (Backend). If the UI is on another port, adjust CORS in [`add-ons/pagi-gateway/src/main.rs`](add-ons/pagi-gateway/src/main.rs) (e.g. `build_app`).

6. **Chat**  
   - Non-streaming: `POST /api/v1/chat` with `{"prompt":"Hello","stream":false}` → JSON with `response`, `thought`, `status`.  
   - Streaming: `POST /api/v1/chat` with `"stream": true` → chunked text (see §2.3.3).  
   - Use `user_alias` (and optionally `tenant_id` on `/v1/execute`) so Kardia and Chronos are tenant-scoped.

7. **Proof of life**  
   In the browser: confirm one concrete UI element (e.g. “Gateway log stream” header, chat input, or status line). Check console for no 404/CORS errors. If you see “Connection Refused”, re-run pre-flight and ensure only one process uses the same `data/` path.

### Summary prompts to run (in order)

- **Backend bring-up:** “Start the PAGI gateway per docs/frontend-backend-integration.md: run pre-flight, then `cargo run -p pagi-gateway`. Confirm /v1/status and /api/v1/health return expected JSON.”  
- **Frontend wiring:** “Wire the frontend to the gateway per docs/frontend-backend-integration.md Phase 2 (drop-in vs Studio). Set API URL to http://127.0.0.1:8001/api/v1/chat for Studio.”  
- **Verification:** “Verify end-to-end per docs/frontend-backend-integration.md Phase 3: CORS, chat (stream and non-stream), and proof of life in the browser.”

---

## 0b) Architecture (mental model)

**Gateway (Bridge)**: Axum HTTP server that exposes the integration API and dispatches requests into the Orchestrator.

* Main entry point: [`add-ons/pagi-gateway/src/main.rs`](add-ons/pagi-gateway/src/main.rs:1)
* Config: [`config/gateway.toml`](config/gateway.toml:1)

**Orchestrator**: Receives a `Goal` (execute a skill, query KB, autonomous goal, etc.) and routes execution.

**KnowledgeStore (8 KBs)**: L2 memory with 8 cognitive slots (Pneuma/Oikos/Logos/Chronos/Techne/Ethos/Kardia/Soma).

* KB overview: [`crates/pagi-core/src/knowledge/mod.rs`](crates/pagi-core/src/knowledge/mod.rs:1)

**MemoryManager (“vault”)**: Long-term sled storage + hot cache for short-term UI state and other tenant-scoped values.

* Vault storage: [`crates/pagi-core/src/memory.rs`](crates/pagi-core/src/memory.rs:1)

---

## 0c) Complete API reference (Gateway routes)

| Method | Path | Purpose | Used by |
|--------|------|---------|---------|
| GET | `/v1/status` | App identity, port, `llm_mode`, `slot_labels` | Drop-in UI, scripts |
| POST | `/v1/execute` | Orchestrator bridge: run a typed `Goal` (e.g. AutonomousGoal, ExecuteSkill) | Drop-in UI ([`pagi-frontend/app.js`](pagi-frontend/app.js)), any client |
| GET | `/v1/research/trace/:trace_id` | Fetch research trace by ID | Research/audit UIs |
| POST | `/v1/vault/read` | Decrypt and return a journal entry (requires `X-Pagi-Shadow-Key` header) | Sovereign Dashboard, secure UIs |
| GET | `/api/v1/health` | Liveness check | Studio UI, scripts |
| GET | `/api/v1/logs` | SSE stream of gateway logs (tracing) | Studio UI Log Terminal |
| POST | `/api/v1/chat` | Chat (stream or JSON); Kardia injection, Chronos persistence | Studio UI ([`apiService.ts`](add-ons/pagi-studio-ui/assets/studio-interface/services/apiService.ts)) |
| GET | `/api/v1/kardia/:user_id` | Current relation/sentiment for user (KB_KARDIA) | Studio UI, verification |
| GET | `/api/v1/kb-status` | Status of all 8 Knowledge Bases | Studio UI Settings / KB panel |
| GET | `/api/v1/sovereign-status` | Full sovereign state (requires `PAGI_API_KEY` if set) | Sovereign Dashboard |

When `frontend_enabled` is true, the gateway also serves the drop-in UI: `/` → `pagi-frontend/index.html`, `/assets/*` and `/ui/*` → `pagi-frontend` directory.

---

## 1) Backend bring-up checklist (required before any Frontend integration)

1. **Confirm the gateway config**
   * Port and storage path in [`config/gateway.toml`](config/gateway.toml:1)
   * Slot label overrides in [`config/gateway.toml`](config/gateway.toml:11)

2. **Start the gateway**
   * Typical dev run: `cargo run -p pagi-gateway` (or whatever wrapper your environment uses)
   * Optional pre-flight checks exist in [`add-ons/pagi-gateway/src/main.rs`](add-ons/pagi-gateway/src/main.rs:77)

3. **Verify bootstraps run** (first start only)
   * Core identity bootstrap (KB-1 / Pneuma): [`initialize_core_identity()`](crates/pagi-core/src/knowledge/bootstrap.rs:30)
   * Core skill registry bootstrap (KB-5 / Techne): [`initialize_core_skills()`](crates/pagi-core/src/knowledge/bootstrap.rs:138)
   * Default ethos policy (KB-6 / Ethos): [`initialize_ethos_policy()`](crates/pagi-core/src/knowledge/bootstrap.rs:245)
   * These are invoked from gateway startup: [`add-ons/pagi-gateway/src/main.rs`](add-ons/pagi-gateway/src/main.rs:147)

4. **Confirm workspace context exists (Oikos)**
   * Gateway will run an initial workspace scan if missing: [`add-ons/pagi-gateway/src/main.rs`](add-ons/pagi-gateway/src/main.rs:180)

---

## 2) API surface a Frontend must integrate

## 2.x Heartbeat (Autonomous Orchestrator) integration notes

The system now includes a **Heartbeat** loop that makes inter-agent messaging event-like (no manual polling).

Current implementation detail:

* The Heartbeat is an **in-process `tokio` task inside the Gateway** (not a separate OS daemon process), so it can share the same `Arc<KnowledgeStore>` without `sled` cross-process file lock contention.
* Source: [`heartbeat_loop()`](add-ons/pagi-gateway/src/main.rs:268) and [`heartbeat_tick()`](add-ons/pagi-gateway/src/main.rs:290)

What it does per tick:

1. Enumerates active `agent_id`s by scanning KB-8/Soma inbox keys (`inbox/{agent_id}/...`).
2. For each agent:
   * If a new inbox message exists, generates an auto-reply using [`ModelRouter.generate_text_raw()`](crates/pagi-skills/src/model_router.rs:264).
   * Pushes the auto-reply back into the sender’s inbox using [`KnowledgeStore.push_agent_message()`](crates/pagi-core/src/knowledge/store.rs:798).
   * Records a reflection event in KB-4/Chronos using [`KnowledgeStore.append_chronos_event()`](crates/pagi-core/src/knowledge/store.rs:711).

Frontend implications:

* A Frontend does **not** need to call [`get_agent_messages`](crates/pagi-skills/src/get_agent_messages.rs:1) to “wake up” agents anymore.
* If your UI surfaces an “Agents / Inbox” panel:
  * Messages will arrive asynchronously (on the next heartbeat tick).
  * The UI can poll `get_agent_messages` for display purposes, but polling is no longer required for agent progress.

Configuration:

* `PAGI_TICK_RATE_SECS` controls pacing (default `5`). Lower values increase responsiveness but may increase LLM usage.
* The Heartbeat currently does **not** delete/ack inbox messages after processing; repeated auto-replies can occur if the same newest message remains newest. If you need exactly-once semantics, add an ack/delete mechanism in KB-8.

### 2.1 GET `/v1/status` (orchestrator identity)

Purpose: “is backend alive”, identity of the app, **slot labels** (what the UI should display for KB slots).

Implementation: [`status()`](add-ons/pagi-gateway/src/main.rs:389)

Response shape (example):

```json
{
  "app_name": "UAC Gateway",
  "port": 8001,
  "llm_mode": "mock",
  "slot_labels": {
    "1": "Brand Voice",
    "2": "Sales",
    "3": "Finance",
    "4": "Operations",
    "5": "Community",
    "6": "Products",
    "7": "Policies",
    "8": "Custom"
  }
}
```

Where slot labels come from: [`config/gateway.toml`](config/gateway.toml:11)

---

### 2.2 POST `/v1/execute` (Orchestrator bridge)

Purpose: a generic “bridge” endpoint that allows a Frontend (or any client) to run a **typed `Goal`**.

Implementation: [`execute()`](add-ons/pagi-gateway/src/main.rs:429)

Request shape:

```json
{
  "tenant_id": "some-user-or-tenant",
  "correlation_id": "optional-trace-id",
  "goal": { "<GoalVariant>": { /* payload */ } }
}
```

Example (Autonomous goal) — used by the simple HTML Frontend:

* Client code: [`runAutonomousGoal()`](pagi-frontend/app.js:1)

```json
{
  "tenant_id": "default",
  "goal": {
    "AutonomousGoal": {
      "intent": "Draft a plan for X",
      "context": null
    }
  }
}
```

**Ethos policy enforcement happens here** (pre-execution scan for `ExecuteSkill`):

* See: [`add-ons/pagi-gateway/src/main.rs`](add-ons/pagi-gateway/src/main.rs:440)

Integration implication:

* Your Frontend should display a user-friendly message if the response is `{"status":"policy_violation"...}`.

---

### 2.3 POST `/api/v1/chat` (UI-friendly chat wrapper)

Purpose: a convenience endpoint for Frontends to send a prompt and get a response from the LLM via the `ModelRouter` skill.

Implementation entry: [`chat()`](add-ons/pagi-gateway/src/main.rs:1043)

Request shape (matches Studio UI):

```json
{
  "prompt": "<user text>",
  "stream": false,
  "user_alias": "optional-user-id",
  "model": "optional-model-override",
  "temperature": 0.2,
  "max_tokens": 500,
  "persona": "optional-persona-string"
}
```

Reference client implementation (Studio UI; base URL is `http://127.0.0.1:8001/api/v1/chat` when using gateway):

* Non-streaming: [`sendMessageToOrchestrator()`](add-ons/pagi-studio-ui/assets/studio-interface/services/apiService.ts:3)
* Streaming client wrapper: [`streamMessageToOrchestrator()`](add-ons/pagi-studio-ui/assets/studio-interface/services/apiService.ts:46)

#### 2.3.0 Emotional Context Layer (Cognitive Governor)

The gateway uses **MentalState** (stored in KB_KARDIA under key `mental_state`) to modulate agent tone:

* **Contextual Grace:** If `relational_stress > 0.7`, the gateway prepends a hidden system instruction so the LLM adopts a supportive, low-pressure, empathetic tone (brevity and reassurance). This applies to `/api/v1/chat` (stream and non-stream) and to the heartbeat auto-reply path.
* **MentalState** is updated by the **JournalSkill** (see §3.4). Raw journal text is never logged or sent to external APIs; only anonymized emotional anchors are used to update scores.
* **ShadowStore (optional):** Sensitive journal entries can be stored encrypted (aes-gcm) when `PAGI_SHADOW_KEY` is set; see `crates/pagi-core/src/shadow_store.rs`.

To update mental state from a Frontend, call `POST /v1/execute` with goal `ExecuteSkill` and name `"JournalSkill"`, payload `{ "raw_text": "user journal text" }`. The skill extracts anonymized anchors and updates MentalState; subsequent chat/heartbeat will use the new tone.

#### 2.3.1 Chat context injection (Kardia)

The gateway injects **relationship context** (Kardia) into prompts if present:

* See: [`RelationRecord.prompt_context()`](crates/pagi-core/src/knowledge/store.rs:369)
* Used during chat: [`add-ons/pagi-gateway/src/main.rs`](add-ons/pagi-gateway/src/main.rs:581)

This produces a prefix like:

```
[Relationship context: User sentiment: <...>. Communication style: <...>. Adjust your tone accordingly.]

<original user prompt>
```

Frontend implication:

* You do **not** need to add this yourself. The backend will add it when it has Kardia data.

#### 2.3.2 Conversation persistence (Chronos)

Chat responses are saved to **KB-4 Chronos** for later recall:

* See: [`save_to_memory()`](add-ons/pagi-gateway/src/main.rs:1103) (non-streaming) and in the streaming handler after the stream completes.

Frontend implication:

* Conversation history persistence is automatic for `/api/v1/chat`.
* If you bypass `/api/v1/chat` and instead call `/v1/execute` directly, you must decide whether you want to persist conversation yourself.

#### 2.3.3 Streaming behavior

`/api/v1/chat` supports streaming via `"stream": true`:

* Streaming handler: [`chat_streaming()`](add-ons/pagi-gateway/src/main.rs:1129)

Important implementation detail:

* The gateway response is currently `Content-Type: text/plain; charset=utf-8` and yields **raw text chunks** (token-ish chunks), **not** `text/event-stream` SSE frames: [`add-ons/pagi-gateway/src/main.rs`](add-ons/pagi-gateway/src/main.rs) (see `chat_streaming` response builder).
* The Studio UI streaming client includes SSE-style parsing (e.g. `data:` lines) for compatibility if the backend is later switched to SSE: [`add-ons/pagi-studio-ui/assets/studio-interface/services/apiService.ts`](add-ons/pagi-studio-ui/assets/studio-interface/services/apiService.ts:79). For the current backend, treat the body as **plain chunked text** and append chunks as they arrive.

Integration options for a new Frontend:

1. Treat streaming as **plain chunked text** and append chunks as they arrive (matches current gateway).
2. If you want SSE end-to-end, change the backend streaming handler to emit SSE events and set `Content-Type: text/event-stream` (then document in this file).

---

### 2.4 GET `/api/v1/sovereign-status` (Sovereign state inspection)

Purpose: Retrieve the full sovereign state for an agent, including trust scores, mental state, and relationship data. This endpoint is designed for the Sovereign Dashboard and other monitoring UIs that need to inspect the agent's internal state when they cannot directly access the Sled database (e.g., when the gateway holds the lock).

Implementation: [`sovereign_status()`](add-ons/pagi-gateway/src/main.rs:892)

**Authentication**: If the `PAGI_API_KEY` environment variable is set, requests must include one of:
* Header `X-API-Key: <key>`
* Header `Authorization: Bearer <key>`

If the key is missing or invalid, the endpoint returns `401 Unauthorized`.

Response shape (example):

```json
{
  "agent_id": "default",
  "trust_score": 0.85,
  "mental_state": {
    "relational_stress": 0.3,
    "cognitive_load": 0.5,
    "emotional_stability": 0.8
  },
  "relationships": {
    "user_123": {
      "sentiment": 0.7,
      "trust": 0.9,
      "communication_style": "professional"
    }
  }
}
```

Frontend integration:

* Use this endpoint when building dashboards or monitoring tools that need to display the agent's internal state.
* Always handle `401 Unauthorized` responses gracefully (prompt for API key or show access denied message).
* The Sovereign Dashboard uses this endpoint when it cannot open the Sled database directly: [`add-ons/pagi-sovereign-dashboard/src/main.rs`](add-ons/pagi-sovereign-dashboard/src/main.rs)

---

### 2.5 POST `/v1/vault/read` (Secure journal entry retrieval)

Purpose: Decrypt and retrieve a specific journal entry from the ShadowStore. This endpoint provides secure access to encrypted journal data for authorized clients.

Implementation: [`vault_read()`](add-ons/pagi-gateway/src/main.rs:955)

**Authentication**: Requires the `X-Pagi-Shadow-Key` header with the same value as the `PAGI_SHADOW_KEY` environment variable. If the key is missing, invalid, or the environment variable is not set, the endpoint returns `403 Forbidden`.

Request shape:

```json
{
  "record_id": "journal_entry_123"
}
```

Response shape (example):

```json
{
  "record_id": "journal_entry_123",
  "label": "anxious",
  "intensity": 0.7,
  "timestamp_ms": 1707253200000,
  "raw_content": "Today was challenging..."
}
```

Error responses:

* `403 Forbidden`: Missing or invalid `X-Pagi-Shadow-Key` header
* `404 Not Found`: Record ID does not exist
* `500 Internal Server Error`: Decryption failed
* `503 Service Unavailable`: ShadowStore not initialized (no `PAGI_SHADOW_KEY` set)

Frontend integration:

* Use this endpoint when building UIs that need to display encrypted journal entries.
* Always store the shadow key securely (never in localStorage or cookies without encryption).
* Handle all error cases gracefully with user-friendly messages.
* Consider implementing a "view journal" feature that prompts for the shadow key on first access.

Security considerations:

* The shadow key must match the server's `PAGI_SHADOW_KEY` environment variable exactly.
* Raw journal content is never logged or sent to external APIs.
* The ShadowStore uses AES-GCM encryption: [`crates/pagi-core/src/shadow_store.rs`](crates/pagi-core/src/shadow_store.rs)

---

## 3) KB (Knowledge Base) integration (8-slot ontology)

### 3.1 Slot model

KB slots are defined in the core and represent cognitive domains:

* Table + meaning: [`crates/pagi-core/src/knowledge/mod.rs`](crates/pagi-core/src/knowledge/mod.rs:5)

Frontend guidance:

* Always display slots using `/v1/status.slot_labels` so UI labels are config-driven.
* Keep internal routing/IDs stable: slots are **1..=8**.

### 3.2 Routing prompt used by the system (Thalamus)

The cognitive router uses an LLM classification prompt to route arbitrary info into exactly one KB domain:

* Prompt template: [`build_classification_prompt()`](crates/pagi-skills/src/thalamus.rs:29)

Frontend implication:

* If the Frontend provides a “save to KB” feature without asking the user to pick a slot, you can:
  1) send the content to a skill that calls Thalamus routing, or
  2) implement a client-side “suggested slot” using the same ontology and ask the backend to confirm.

### 3.3 How to query/insert KB data from a Frontend

Mechanically this happens through `/v1/execute` goals.

Conceptual patterns:

* **QueryKnowledge**: read a key from a slot
* **UpdateKnowledgeSlot / KnowledgeInsert**: add/update records
* **ExecuteSkill**: use skills as an API layer over KB operations

Because the exact `Goal` JSON tagging is Rust-serde-driven, treat the canonical contract as:

* Backend enum: `Goal` (see crate exports in [`add-ons/pagi-gateway/src/main.rs`](add-ons/pagi-gateway/src/main.rs:18))

Practical integration approach:

* Start by integrating `/api/v1/chat` first.
* Then integrate KB read/write via specific skills (easier to keep stable) instead of directly hand-crafting `Goal` variants.

---

## 4) Memory integration (Vault + UI state)

There are two relevant “memory” layers for Frontends:

1. **Vault (MemoryManager)** — tenant-scoped paths and values (hot cache + sled): [`MemoryManager`](crates/pagi-core/src/memory.rs:16)
2. **Chronos (KB-4)** — conversation history and episodic events (stored in KnowledgeStore)

### 4.1 Reference pattern: Studio “prompt/response maps to short-term memory”

The Studio add-on explicitly maps UI state into short-term memory paths:

* Constants: [`MEMORY_PROMPT_PATH`](add-ons/pagi-studio-ui/src/app.rs:80), [`MEMORY_RESPONSE_PATH`](add-ons/pagi-studio-ui/src/app.rs:81)

Integration takeaway:

* For any new Frontend, define a small, explicit set of **memory paths** that represent “UI session state” (last prompt, last response, selected model, user preferences).
* Store those values via a backend memory API (if/when exposed) or via a dedicated skill.

### 4.2 Conversation persistence (Chronos)

If you use `/api/v1/chat`, the gateway stores the user/assistant exchange as a `KbRecord` with metadata in KB-4/Chronos:

* Implementation: [`save_to_memory()`](add-ons/pagi-gateway/src/main.rs:731)

---

## 5) Backend “prompt inventory” (what the system injects/uses)

This section is the authoritative index of prompts that materially affect responses.

### 5.1 Thalamus routing prompt

* Prompt text: [`build_classification_prompt()`](crates/pagi-skills/src/thalamus.rs:29)
* Purpose: classify content → exactly one KB domain

### 5.2 Kardia relationship context prompt-prefix

* Prefix builder: [`RelationRecord.prompt_context()`](crates/pagi-core/src/knowledge/store.rs:369)
* Purpose: tone/communication-style adaptation

### 5.3 Skill-registry appendix (ModelRouter)

ModelRouter can append a system “skills list” (from KB-5/Techne) to prompts:

* Entry point: [`ModelRouter::with_knowledge()`](crates/pagi-skills/src/model_router.rs:136)
* Skills appendix builder: `build_system_prompt_from_skills()` in [`crates/pagi-skills/src/model_router.rs`](crates/pagi-skills/src/model_router.rs:153)

Frontend implication:

* If you want the model to be aware of available skills, ensure the backend is running ModelRouter in a mode that includes the KB-5 appendix.

### 5.4 Identity/persona bootstraps (KB-1)

These are not “prompts” in the strict sense, but they are **core instruction data** stored at bootstrap:

* Bootstrap function: [`initialize_core_identity()`](crates/pagi-core/src/knowledge/bootstrap.rs:30)
* Identity keys (mission, priorities, persona): [`crates/pagi-core/src/knowledge/bootstrap.rs`](crates/pagi-core/src/knowledge/bootstrap.rs:9)

---

## 6) “Integration Prompts” (copy/paste SOP prompts)

These are operational prompts intended for a developer (or an LLM acting as a dev assistant) when integrating a new Frontend. Use them in the order below when tying backend, gateway, engine, and frontend together.

### 6.0 Master end-to-end integration prompt (full stack)

Use this when you need one prompt that covers the entire path from backend to UI:

```
Wire the PAGI backend, gateway, and engine to the frontend UI end-to-end.

Backend & gateway:
1) Run pre-flight: cargo run -p pagi-gateway -- --verify (port 8001, no Sled locks).
2) Start gateway: cargo run -p pagi-gateway from workspace root. Confirm GET /v1/status and GET /api/v1/health return expected JSON.
3) Slot labels and app identity come from config/gateway.toml; the UI should load them from GET /v1/status.

Frontend options:
- Drop-in UI: gateway serves pagi-frontend at http://127.0.0.1:8001/ when frontend_enabled=true. Frontend uses POST /v1/execute for goals (e.g. AutonomousGoal).
- Studio UI: run gateway on 8001 and npm run dev in add-ons/pagi-studio-ui/assets/studio-interface (port 3001). Set API URL to http://127.0.0.1:8001/api/v1/chat. Logs: http://127.0.0.1:8001/api/v1/logs.

API contract:
- Chat: POST /api/v1/chat with { prompt, stream, user_alias, model, temperature, max_tokens, persona }. Non-stream returns JSON; stream returns plain chunked text (Content-Type: text/plain).
- Execute: POST /v1/execute with { tenant_id, correlation_id?, goal }. Surface policy_violation and error status in the UI.
- Health: GET /api/v1/health. KB status: GET /api/v1/kb-status. Kardia: GET /api/v1/kardia/:user_id.

Verification:
- CORS allows Frontend ports 3001–3099 and Backend 8001–8099.
- In browser: confirm one concrete UI element and zero 404/CORS errors. If Connection Refused, re-run pre-flight and ensure only one process uses the same data/ path.

Deliver: a short runbook (steps + commands + URLs) and any code changes needed for this project.
```

### 6.1 Frontend integration kickoff prompt

Use this when starting a new UI integration task:

```
You are integrating a new Frontend UI with the PAGI backend gateway.

Requirements:
1) Implement GET /v1/status for app identity and slot label hydration; GET /api/v1/health for liveness.
2) Implement POST /api/v1/chat for non-streaming chat (JSON request/response).
3) Implement streaming chat using the backend’s current behavior (plain chunked text; Content-Type: text/plain) OR document + implement SSE consistently end-to-end.
4) Surface policy_violation and error responses from POST /v1/execute.
5) Use user_alias (chat) and tenant_id (execute) consistently so Kardia and Chronos are tenant-scoped.

Provide:
* A minimal API client module (base URL configurable, e.g. http://127.0.0.1:8001)
* UI wiring examples for status, chat, and execute
* Error handling patterns
* A short integration checklist for QA
```

### 6.2 Bridge/Gateway integration prompt (when the UI fails to connect)

```
Diagnose why the Frontend cannot integrate with the PAGI gateway.

Check:
* Gateway is listening on the configured port (config/gateway.toml; default 8001).
* CORS in add-ons/pagi-gateway/src/main.rs allows the Frontend origin (ports 3001–3099 and 8001–8099).
* GET /v1/status returns expected JSON (app_name, port, llm_mode, slot_labels).
* GET /api/v1/health returns {"status":"ok"}.
* POST /api/v1/chat with stream=false returns JSON (response, thought, status).
* For stream=true, the gateway sends raw text chunks (Content-Type: text/plain), not SSE.

Return:
* Root cause
* Concrete fixes (config + code pointers)
* Verification steps (curl or browser)
```

### 6.3 KB integration prompt (adding a “save to knowledge” feature)

```
Add a Frontend feature to store and retrieve knowledge.

Constraints:
* The backend has 8 KB slots; UI must display slot labels from GET /v1/status (slot_labels).
* Use GET /api/v1/kb-status for status of all 8 Knowledge Bases.
* Prefer calling stable skills via POST /v1/execute rather than hardcoding Rust Goal JSON variants.

Deliver:
* A UI flow: pick slot (or route via Thalamus), choose key, write record, then read it back.
* A test plan that verifies data persists across gateway restarts.
```

### 6.4 Memory integration prompt (persist UI state)

```
Integrate short-term memory for the UI.

Goal:
* Persist last prompt, last response, selected model, and user settings per tenant.

Reference:
* Studio uses memory paths like "studio/last_prompt" and "studio/last_response" (see add-ons/pagi-studio-ui/src/app.rs MEMORY_PROMPT_PATH / MEMORY_RESPONSE_PATH).

Deliver:
* A list of memory paths for this Frontend
* A backend interaction plan (memory API or skill)
* A migration plan for future schema changes
```

### 6.5 Verification and proof-of-life prompt

```
Verify the frontend–backend integration is working.

Steps:
1) Run cargo run -p pagi-gateway -- --verify; then cargo run -p pagi-gateway.
2) For Studio UI: in another terminal, npm run dev in add-ons/pagi-studio-ui/assets/studio-interface; open http://127.0.0.1:3001; set API URL to http://127.0.0.1:8001/api/v1/chat.
3) In the browser: confirm the Log Terminal connects to http://127.0.0.1:8001/api/v1/logs and shows gateway logs.
4) Send a chat message; confirm response and no console errors.
5) Provide proof of life: name one specific UI element you see (e.g. "Gateway log stream" header, chat input, CONNECTED status).
```

---

## 7) Integration checklist (for new Frontends)

Follow **§0) Step-by-step integration** for the full sequence. Minimum viable integration:

1. Call `GET /v1/status`; show `app_name`, `llm_mode`, and slot labels. Optionally call `GET /api/v1/health` for liveness.
2. Implement `POST /api/v1/chat` non-streaming request/response (JSON).
3. Add tenant identity: set `user_alias` (chat) and/or `tenant_id` (execute).
4. Show errors clearly (`status=error`, `status=policy_violation`).
5. If streaming is enabled, implement chunked streaming UI updates (current gateway sends plain text chunks).

Full-feature integration:

6. Add “KB panel” UI (8 slots + labels from `/v1/status`, status from `GET /api/v1/kb-status`).
7. Add “save to KB” and “search/query KB” flows (via `/v1/execute` goals or skills).
8. Add “logs/traces” view: connect to `GET /api/v1/logs` (SSE) for gateway logs.
9. Add memory-backed UI state (last prompt/response, pinned items, settings) per §4.

Verification: run pre-flight, start gateway, then confirm in browser one concrete UI element and zero 404/CORS errors (§0 Phase 3 and prompt §6.5).

---

## 8) Change management

This file is intended to be updated as the backend contract evolves.

Process:

* Any time `/v1/*` or `/api/v1/*` contracts change, update this doc in the same PR.
* When streaming protocol changes (plain chunked text ↔ SSE), update:
  * the backend handler ([`chat_streaming()`](add-ons/pagi-gateway/src/main.rs:1129))
  * the reference client ([`streamMessageToOrchestrator()`](add-ons/pagi-studio-ui/assets/studio-interface/services/apiService.ts:46))
  * §2.3.3 and this document.

