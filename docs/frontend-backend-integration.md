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

## 0) Architecture (mental model)

**Gateway (Bridge)**: Axum HTTP server that exposes the integration API and dispatches requests into the Orchestrator.

* Main entry point: [`add-ons/pagi-gateway/src/main.rs`](add-ons/pagi-gateway/src/main.rs:1)
* Config: [`config/gateway.toml`](config/gateway.toml:1)

**Orchestrator**: Receives a `Goal` (execute a skill, query KB, autonomous goal, etc.) and routes execution.

**KnowledgeStore (8 KBs)**: L2 memory with 8 cognitive slots (Pneuma/Oikos/Logos/Chronos/Techne/Ethos/Kardia/Soma).

* KB overview: [`crates/pagi-core/src/knowledge/mod.rs`](crates/pagi-core/src/knowledge/mod.rs:1)

**MemoryManager (“vault”)**: Long-term sled storage + hot cache for short-term UI state and other tenant-scoped values.

* Vault storage: [`crates/pagi-core/src/memory.rs`](crates/pagi-core/src/memory.rs:1)

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

### 2.1 GET `/v1/status`

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

### 2.3 POST `/v1/chat` (UI-friendly chat wrapper)

Purpose: a convenience endpoint for Frontends to send a prompt and get a response from the LLM via the `ModelRouter` skill.

Implementation entry: [`chat()`](add-ons/pagi-gateway/src/main.rs:555)

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

Reference client implementation:

* Non-streaming: [`sendMessageToOrchestrator()`](add-ons/pagi-studio-ui/assets/studio-interface/services/apiService.ts:3)
* Streaming client wrapper: [`streamMessageToOrchestrator()`](add-ons/pagi-studio-ui/assets/studio-interface/services/apiService.ts:47)

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

* See: [`save_to_memory()`](add-ons/pagi-gateway/src/main.rs:731)

Frontend implication:

* Conversation history persistence is automatic for `/v1/chat`.
* If you bypass `/v1/chat` and instead call `/v1/execute` directly, you must decide whether you want to persist conversation yourself.

#### 2.3.3 Streaming behavior

`/v1/chat` supports streaming via `"stream": true`:

* Streaming handler: [`chat_streaming()`](add-ons/pagi-gateway/src/main.rs:639)

Important implementation detail:

* The response is currently `Content-Type: text/plain` and yields **raw text chunks** (token-ish chunks), **not** `text/event-stream` SSE frames: [`add-ons/pagi-gateway/src/main.rs`](add-ons/pagi-gateway/src/main.rs:722)
* The Studio UI streaming client currently includes SSE-frame parsing logic: [`add-ons/pagi-studio-ui/assets/studio-interface/services/apiService.ts`](add-ons/pagi-studio-ui/assets/studio-interface/services/apiService.ts:79)

Integration options for a new Frontend:

1. Treat streaming as **plain chunked text** and append chunks as they arrive.
2. If you want SSE, adjust backend streaming to emit SSE events and set `Content-Type: text/event-stream` (requires code change; document this decision if you do it).

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

* Start by integrating `/v1/chat` first.
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

If you use `/v1/chat`, the gateway stores the user/assistant exchange as a `KbRecord` with metadata in KB-4/Chronos:

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

These are operational prompts intended for a developer (or an LLM acting as a dev assistant) when integrating a new Frontend.

### 6.1 Frontend integration kickoff prompt

Use this when starting a new UI integration task:

```
You are integrating a new Frontend UI with the PAGI backend gateway.

Requirements:
1) Implement GET /v1/status health + slot label hydration.
2) Implement POST /v1/chat for non-streaming chat.
3) Implement streaming chat using the backend’s current behavior (chunked text) OR document + implement SSE consistently end-to-end.
4) Surface policy_violation responses from POST /v1/execute.
5) Use user_alias/tenant_id consistently so Kardia and Chronos are tenant-scoped.

Provide:
* A minimal API client module
* UI wiring examples
* Error handling patterns
* A short integration checklist for QA
```

### 6.2 Bridge/Gateway integration prompt (when the UI fails to connect)

```
Diagnose why the Frontend cannot integrate with the PAGI gateway.

Check:
* Gateway is listening on the configured port (config/gateway.toml).
* CORS settings allow the Frontend origin.
* /v1/status returns expected JSON.
* /v1/chat returns JSON for stream=false.
* For stream=true, confirm whether the gateway is sending raw text chunks or SSE frames.

Return:
* Root cause
* Concrete fixes (config + code pointers)
* Verification steps
```

### 6.3 KB integration prompt (adding a “save to knowledge” feature)

```
Add a Frontend feature to store and retrieve knowledge.

Constraints:
* The backend has 8 KB slots; UI must display slot labels from /v1/status.
* Prefer calling stable skills via /v1/execute rather than hardcoding Rust Goal JSON variants.

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
* Studio uses memory paths like "studio/last_prompt" and "studio/last_response".

Deliver:
* A list of memory paths for this Frontend
* A backend interaction plan (memory API or skill)
* A migration plan for future schema changes
```

---

## 7) Integration checklist (for new Frontends)

Minimum viable integration:

1. Call `/v1/status`; show `app_name`, `llm_mode`, and slot labels.
2. Implement `/v1/chat` non-streaming request/response.
3. Add tenant identity: set `user_alias` (chat) and/or `tenant_id` (execute).
4. Show errors clearly (`status=error`, `status=policy_violation`).
5. If streaming is enabled, implement chunked streaming UI updates.

Full-feature integration:

6. Add “KB panel” UI (8 slots + label hydration).
7. Add “save to KB” and “search/query KB” flows.
8. Add “logs/traces” view (if you expose gateway logs via SSE).
9. Add memory-backed UI state (last prompt/response, pinned items, settings).

---

## 8) Change management

This file is intended to be updated as the backend contract evolves.

Process:

* Any time `/v1/*` contracts change, update this doc in the same PR.
* When streaming protocol changes (plain chunked text ↔ SSE), update:
  * the backend handler ([`chat_streaming()`](add-ons/pagi-gateway/src/main.rs:639))
  * the reference client ([`streamMessageToOrchestrator()`](add-ons/pagi-studio-ui/assets/studio-interface/services/apiService.ts:47))
  * this document.

