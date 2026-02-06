# PAGI Studio UI

The web interface for the PAGI Master Orchestrator ecosystem.

## Architecture

```
┌─────────────────┐      HTTP/REST      ┌─────────────────┐      OpenRouter      ┌─────────────┐
│   Studio UI     │ ──────────────────► │   pagi-gateway  │ ──────────────────► │   LLM API   │
│   (React/Vite)  │      Port 3001      │   (Rust/Axum)   │      Port 8001      │  (External) │
│   NO API KEYS   │ ◄────────────────── │   ALL API KEYS  │ ◄────────────────── │             │
└─────────────────┘                     └─────────────────┘                     └─────────────┘
```

**All LLM calls are routed through the Rust backend.** The frontend has no API keys.

## Prerequisites

- Node.js (v18+)
- Running `pagi-gateway` on port 8001

## Run Locally

1. **Start the backend first:**
   ```bash
   # From project root
   cargo run -p pagi-gateway
   ```

2. **Install frontend dependencies:**
   ```bash
   npm install
   ```

3. **Start the dev server:**
   ```bash
   npm run dev
   ```

4. **Open** http://127.0.0.1:3001

## Configuration

The UI connects to the backend at `http://127.0.0.1:8001/api/v1/chat` by default.
You can change this in the Settings panel (gear icon) under "Orchestrator Endpoint".

## Port Map

| Service | Port | Purpose |
|---------|------|---------|
| Gateway | 8001 | Rust API backend |
| Studio UI | 3001 | React dev server |

## No Frontend API Keys

This UI does **not** require any `.env` file. All API keys are configured in the **root `.env`** file and used by the Rust backend only.
