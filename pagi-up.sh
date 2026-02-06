#!/bin/bash
# PAGI Ecosystem Startup Script (Unix / macOS / Git Bash)
# Order: 1) Build, 2) Gateway (background), 3) Control Panel (background), 4) Studio UI (foreground).
# On exit (e.g. Ctrl+C on Studio UI), background processes are killed.

set -e
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

echo -e "\033[36m--- Starting PAGI Master Orchestrator Ecosystem ---\033[0m"

# 0. Force Kill: clear PAGI ports so no zombie process blocks the build or bind
echo -e "\033[35mClearing PAGI ports (8001, 8002, 3001)...\033[0m"
for port in 8001 8002 3001; do
  fuser -k "${port}/tcp" 2>/dev/null || true
done

# 1. Build
echo -e "\033[33m[1/4] Checking workspace integrity...\033[0m"
cargo build --workspace
echo -e "\033[32m[2/4] Launching pagi-gateway...\033[0m"
cargo run -p pagi-gateway &
GATEWAY_PID=$!

echo -e "\033[32m[3/4] Launching pagi-control-panel...\033[0m"
cargo run -p pagi-control-panel &
PANEL_PID=$!

echo -e "\033[32m[4/4] Launching pagi-studio-ui...\033[0m"

cleanup() {
  echo -e "\033[36mShutting down gateway and control panel...\033[0m"
  kill -- $GATEWAY_PID $PANEL_PID 2>/dev/null || true
  wait $GATEWAY_PID $PANEL_PID 2>/dev/null || true
}
trap cleanup EXIT INT TERM

cargo run -p pagi-studio-ui --bin pagi-studio-ui
