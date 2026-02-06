# PAGI Ecosystem Startup Script (Windows)
# Order: 1) Build, 2) Gateway (new window), 3) Control Panel (new window), 4) Studio UI (foreground)

$ErrorActionPreference = "Stop"

Write-Host "--- Starting PAGI Master Orchestrator Ecosystem ---" -ForegroundColor Cyan

# 0. Force Kill: clear PAGI ports so no zombie process blocks the build or bind
$ports = @(8001, 8002, 3001)
foreach ($port in $ports) {
    $conn = Get-NetTCPConnection -LocalPort $port -ErrorAction SilentlyContinue
    if ($conn) {
        $procId = $conn.OwningProcess | Select-Object -First 1
        if ($procId) {
            Write-Host "Cleaning port $port (PID: $procId)..." -ForegroundColor Magenta
            Stop-Process -Id $procId -Force -ErrorAction SilentlyContinue
        }
    }
}

# 1. Build the workspace
Write-Host "[1/4] Checking workspace integrity..." -ForegroundColor Yellow
Set-Location $PSScriptRoot
cargo build --workspace
if ($LASTEXITCODE -ne 0) { Write-Host "Build failed. Aborting." -ForegroundColor Red; exit 1 }

# 2. Start the Gateway (Backend) in a separate window
Write-Host "[2/4] Launching pagi-gateway..." -ForegroundColor Green
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PSScriptRoot'; cargo run -p pagi-gateway"

# 3. Start the Control Panel (System Toggles) in a separate window
Write-Host "[3/4] Launching pagi-control-panel..." -ForegroundColor Green
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PSScriptRoot'; cargo run -p pagi-control-panel"

# 4. Start the Studio UI (User Interface) in this window
Write-Host "[4/4] Launching pagi-studio-ui..." -ForegroundColor Green
cargo run -p pagi-studio-ui --bin pagi-studio-ui

Write-Host "--- Studio UI closed. Gateway and Control Panel are still running in their own windows. Close those when done. ---" -ForegroundColor Cyan
