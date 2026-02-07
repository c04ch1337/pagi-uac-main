# Universal AGI Core (UAC)

A high-performance agentic system built in **Rust**, **bare-metal** (no Docker), with multi-layer memory, 8 knowledge bases, and a dynamic Skills registry.

---

## System configuration

### Network topology

| Range | Purpose |
|-------|---------|
| **8001–8099** | Backend/API — Rust services (Gateway, Orchestrator, Skills, KB APIs) |
| **3001–3099** | Frontend/UI — Web interfaces and dashboards (Vite dev, Studio UI server) |

### Infrastructure

- **Deployment:** Bare metal only (no containers).
- **Core engine:** Rust (latest stable).
- **Memory architecture:**
  - **L1 (short-term):** Fast-access cache / in-memory (e.g. DashMap).
  - **L2 (long-term):** Persistent storage (Sled).
- **Intelligence:** Master Orchestrator with 8 Knowledge Bases and a modular Skills-type registry.

---

## Workspace layout

| Path | Role |
|------|------|
| **`crates/pagi-core`** | Core library: orchestrator, memory (Sled + DashMap), 8-slot knowledge store, control-panel protocol (`ControlPanelMessage`). |
| **`crates/pagi-skills`** | Trait-based skill registry: LeadCapture, KnowledgeQuery, KnowledgeInsert, CommunityPulse, DraftResponse, ModelRouter, ResearchAudit, CommunityScraper, SalesCloser, KnowledgePruner. |
| **`add-ons/pagi-gateway`** | Axum API gateway: `POST /v1/execute`, `GET /v1/status`, serves `pagi-frontend` when enabled. |
| **`add-ons/pagi-control-panel`** | egui window: KB toggles (1–8), skills on/off, memory weights; sends `ControlPanelMessage` to the orchestrator. |
| **`add-ons/pagi-studio-ui`** | Developer cockpit (eframe): prompt/response, 8 KB sidebar with descriptive names, control bar (same state as control panel), **Skill Tester** (fire any skill with raw JSON), optional HTTP server for the React “Studio” web UI. |
| **`add-ons/pagi-companion-ui`**, **pagi-offsec-ui**, **pagi-personal-ui** | Additional egui add-ons. |
| **`config/`** | `gateway.toml`, `blueprint.json` (intent → skill chains). |
| **`pagi-frontend/`** | Static web UI served by the gateway when `frontend_enabled: true`. |

---

## 8 knowledge bases (Sled)

Initialized via `pagi_init_kb_metadata()`; labels are defined in `pagi-core` and shown in the Studio UI sidebar:

1. Marketing & Brand Identity  
2. Technical Documentation & Code  
3. Financial & Market Data  
4. Project Management & Blueprints  
5. Legal & Compliance  
6. Research & Internal Testing  
7. Strategy & Logistics  
8. Custom/Overflow  

---

## Quick start

**Build and test**

```bash
cargo build --workspace
cargo test --workspace
```

**Run the gateway only**

```bash
cargo run -p pagi-gateway
# Listens on 127.0.0.1:8001 (config/gateway.toml). Run from repo root so config/ and data/ resolve.
```

**Run the full stack (gateway + control panel + Studio UI)**

- **Windows (PowerShell):**  
  `.\pagi-up.ps1`  
  Starts the gateway and control panel in separate windows, then launches the Studio UI in the current window.

- **Linux / macOS / Git Bash:**  
  `chmod +x pagi-up.sh && ./pagi-up.sh`  
  Starts the gateway and control panel in the background and runs the Studio UI in the foreground; exiting the Studio UI stops the background processes.

**Studio UI only (no gateway)**

```bash
cargo run -p pagi-studio-ui
# Uses local data/ (pagi_vault, pagi_knowledge); control bar and Skill Tester work against the in-process orchestrator.
```

**Google Studio–style web UI (React)**

```bash
cd add-ons/pagi-studio-ui/assets/studio-interface && npm install && npm run build
cargo run -p pagi-studio-ui --bin pagi-studio-ui-server
# Serves the built app and API on http://127.0.0.1:3001 and opens the browser.
```

---

## Studio UI highlights

- **Left sidebar:** 8 KBs with descriptive names and active/inactive state (from `pagi_core` control state).
- **Control bar (bottom):** KB toggles and Skills ON/OFF; same `ControlPanelMessage` stream as the Control Panel add-on.
- **Skill Tester (collapsible):** Choose a skill from the dropdown, paste raw JSON (e.g. for CommunityScraper), click **Execute Skill (Fire)**. Execution runs in a worker thread (no UI freeze); result and timing (ms) are shown in the output inspector.
- **Prompt / Send:** Dispatches `Goal::QueryKnowledge` (or other goals) via the orchestrator.

---

## Port map and troubleshooting

| Component | Port | Range | Purpose |
|-----------|------|-------|---------|
| **pagi-gateway** | **8001** | Backend 8001–8099 | Brain API and orchestrator entry point. Binds to 127.0.0.1. |
| **pagi-studio-ui-server** | **3001** | Frontend 3001–3099 | Rust add-on that serves the built React app and bridges to the Gateway. |
| **Vite dev** (Studio React) | **3001** | Frontend 3001–3099 | Local dev server for the React Studio interface. |

CORS on the gateway allows origins in the Backend (8001–8099) and Frontend (3001–3099) port ranges.

**If the UI shows "Connection Error" or stays blank:**

1. **Verify the gateway:** Open `http://127.0.0.1:8001/api/v1/health` in a browser. If it fails, the gateway isn’t running (check for Sled DB lock errors in the terminal). If it returns `{"status":"ok"}`, the backend is fine; the issue is UI configuration.
2. **Check the Studio API URL:** In Settings (Orchestrator Endpoint), use `http://127.0.0.1:3001/api/v1/chat` when using the UI server (same-origin), or `http://127.0.0.1:8001` (direct to gateway) if the React app is served from elsewhere.
3. **Clear port conflicts:** If you see "Address already in use", stop the process on that port before starting again.
   - **Windows (PowerShell):**  
     `Stop-Process -Id (Get-NetTCPConnection -LocalPort 8001).OwningProcess -Force` (repeat for 3001, 8002 as needed).
   - **Linux/macOS:**  
     `fuser -k 8001/tcp` (repeat for 3001, 8002 as needed).

---

## Config and data

- **Gateway:** `config/gateway.toml` (or `PAGI_CONFIG`); `config/blueprint.json` (or `PAGI_BLUEPRINT_PATH`). Storage path defaults to `./data` (Sled: `pagi_vault`, `pagi_knowledge`).
- Run the gateway and Studio UI from the **repository root** so relative paths resolve.

---

## 🚀 Deployment

For a **stable, persistent** bare-metal deployment (environment variables, security hardening, memory locking for Slot 9, and service setup), see the full guide:

- **[docs/DEPLOYMENT.md](docs/DEPLOYMENT.md)** — Prerequisites, env template (`PAGI_CONFIG`, `PAGI_SHADOW_KEY`, `PAGI_API_KEY`, storage path), Slot 9 (Shadow) memory-locking, execution flow, and systemd/Windows Task or Service examples.

**Quick Start — system integrity check:** After building and starting the gateway, verify the sanctuary with the Sovereign Dashboard:

```bash
cargo build --release
# Start gateway (e.g. cargo run -p pagi-gateway --release), then:
cargo run -p pagi-sovereign-dashboard --release -- status
# or: ./target/release/pagi status
```

---

## Docs

- `docs/BARE_METAL_ARCHITECTURE.md` — Architecture overview.  
- `docs/DEPLOYMENT.md` — **Bare-metal deployment guide** (env, security, services).  
- `docs/PROJECT_ANATOMY.md` — Project structure.  
- `docs/WORKSPACE_HEALTH_REPORT.md` — Post-migration workspace verification.
