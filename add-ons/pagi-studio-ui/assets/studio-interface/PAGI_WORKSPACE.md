# PAGI Studio — Web UI (workspace-integrated)

This folder contains the **Google AI Studio–style web interface** migrated into the PAGI workspace under `add-ons/pagi-studio-ui`.

## Layout

- **Paths are relative to the workspace root.** This directory is at:
  `add-ons/pagi-studio-ui/assets/studio-interface/`
- The **native PAGI Studio** (eframe) lives in `add-ons/pagi-studio-ui/src/` and calls `pagi_core::Orchestrator::dispatch()` directly.
- This **web UI** talks to a backend over HTTP. In Settings you can set the API URL.

## Backend

- **Default** (this app): expects a `/chat/stream`-style API (prompt, stream, persona, etc.).
- **PAGI Gateway** (same workspace): runs at `http://localhost:8001` (Backend/API range 8001-8099) and exposes:
  - `GET /v1/status` — app identity and slot labels
  - `POST /v1/execute` — JSON body with `tenant_id` and `goal` (e.g. `QueryKnowledge`, `IngestData`, `AutonomousGoal`).

To use this web UI with the PAGI Gateway you need either:

1. A small proxy that maps `/chat/stream` requests to `/v1/execute` (Goal JSON), or  
2. Use the **native** `pagi-studio-ui` binary (`cargo run -p pagi-studio-ui`), which is wired to `pagi_core::Orchestrator` and the 8-slot knowledge base.

## Build and run (web UI)

From this directory:

```bash
npm install
npm run dev
```

Then open the URL shown (e.g. http://localhost:3001). Set the API URL in Settings (gear icon) to your backend (e.g. http://127.0.0.1:8001 or http://127.0.0.1:3001/api/v1/chat).

## Assets

All assets for this web UI live under this folder. The Rust crate’s `config` loads `ui_config.json` from `add-ons/pagi-studio-ui/assets/` (sibling to this `studio-interface` directory).
