# PAGI Frontend Design System

All add-on frontends **must** call `pagi_core::Orchestrator::dispatch()` for AGI reasoning. Each add-on uses a different UI stack and focus.

---

## Primary UI (Reference Implementation)

- **`add-ons/pagi-studio-ui`** is the **Developer's Cockpit** — the **Standard Reference Frontend** (Google AI Studio style). New features (new KBs, memory layers) must be implemented and tested in **pagi-studio-ui first**; once stable, they can be ported to specialized add-ons.

---

## Add-on Stack & Focus

| Add-on | Stack | Focus |
|--------|--------|--------|
| **pagi-studio-ui** | **Egui** (Developer's Cockpit) | Reference implementation; Prompt/Response ↔ memory; 8 KB status panel |
| **pagi-companion-ui** | High-fidelity Rust GUI (**Slint**) | Character-engine traits, avatar rendering |
| **pagi-offsec-ui** | **Ratatui** (TUI) | Raw data streams, network logs, keyboard-driven navigation |
| **pagi-personal-ui** | Lightweight dashboard (**Egui**) | System tray integration, minimal resource usage |

---

## pagi-companion-ui (Slint)

- **Technology:** Slint 1.x.
- **Focus:** Character-engine traits and avatar rendering; high-fidelity native or embedded UI.
- **AGI:** Button or flow triggers `Orchestrator::dispatch()` (e.g. `QueryKnowledge`, `AutonomousGoal`); result shown in UI. Dispatch runs off the UI thread and updates via Slint’s event loop.
- **Run:** `cargo run -p pagi-companion-ui` (from repo root). Storage under `./data` (current_dir-relative).

---

## pagi-offsec-ui (Ratatui)

- **Technology:** Ratatui + crossterm.
- **Focus:** Raw data streams, network logs, keyboard-driven navigation (no mouse required).
- **AGI:** Key **R** (or similar) triggers `Orchestrator::dispatch()`; result appended to the log/stream view.
- **Run:** `cargo run -p pagi-offsec-ui`. **Q** quits. Storage under `./data`.

---

## pagi-personal-ui (Egui)

- **Technology:** eframe/egui with Glow backend.
- **Focus:** Lightweight dashboard, minimal resource usage; system tray can be added (e.g. via `tray-icon` crate) for minimize-to-tray.
- **AGI:** “Dispatch” button calls `Orchestrator::dispatch()`; result shown in a scrollable area.
- **Run:** `cargo run -p pagi-personal-ui`. Storage under `./data`.

---

## Rule: Orchestrator::dispatch()

Every frontend must perform AGI reasoning **only** through:

```rust
orchestrator.dispatch(&tenant_ctx, goal).await
```

Goals (e.g. `QueryKnowledge`, `IngestData`, `AutonomousGoal`) are defined in `pagi_core::Goal`. Add-ons build the orchestrator (memory + knowledge + skills + blueprint) from local paths (e.g. `current_dir()/data`) so everything stays **bare-metal** and **UI-agnostic** in `crates/pagi-core`.

---

## pagi-studio-ui (Developer's Cockpit)

- **Technology:** eframe/egui with Glow backend.
- **Focus:** Reference implementation; Prompt and Response fields mapped to short-term memory (`studio/last_prompt`, `studio/last_response`); 8 KB status panel (key counts per slot); `ui_config.json` and `assets/` loaded locally (no CDNs).
- **AGI:** "Send (Dispatch)" runs `Orchestrator::dispatch(QueryKnowledge { slot_id, query })`; result shown in Response and persisted to memory. KB panel shows status for slots 1–8.
- **Run:** `cargo run -p pagi-studio-ui`. Storage under `./data`; config from `assets/ui_config.json`.
