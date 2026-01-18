#!/bin/bash
# start_mission.sh
# Orchestrates the startup of PROJECT OPB (Rust Backend + Astro Frontend)

# Function to kill child processes on exit
cleanup() {
    echo ""
    echo "🛑 Mission Abort Initiated. Shutting down systems..."
    kill -- -$$ 2>/dev/null
    exit 0
}

# Trap interrupts for cleanup
trap cleanup SIGINT SIGTERM

echo "🚀 Initiating Mission Launch Sequence (OPB)..."
echo "============================================"

# Get the script's directory (project root)
PROJECT_ROOT="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
BACKEND_DIR="$PROJECT_ROOT/backend"

# 1. Start Rust Backend Services
echo "⚙️  Starting Rust Backend Core..."
if [ -d "$BACKEND_DIR" ]; then
    cd "$BACKEND_DIR"
    
    # Build first
    echo "   [Building Rust Binaries...]"
    
    # Clean up existing processes and locks
    pkill -f 'target/release/server' 2>/dev/null || true
    pkill -f 'target/release/bridge' 2>/dev/null || true
    pkill -f 'target/release/sentry' 2>/dev/null || true
    pkill -f 'target/debug/server' 2>/dev/null || true
    pkill -f 'target/debug/bridge' 2>/dev/null || true
    pkill -f 'target/debug/sentry' 2>/dev/null || true
    
    # Remove stale browser lock
    rm -f browser_data/SingletonLock 2>/dev/null || true
    sleep 1
    
    cargo build --release --bins --quiet
    
    # Start Server Only (as requested)
    cargo run --release --bin server &
    SERVER_PID=$!
    echo "   → Server Module (PID: $SERVER_PID)"
    
    # Start Browser Bridge
    cargo run --release --bin bridge &
    BRIDGE_PID=$!
    echo "   → Bridge Module (PID: $BRIDGE_PID)"
    
    # Activate Sentry
    cargo run --release --bin sentry &
    SENTRY_PID=$!
    echo "   → Sentry Module (PID: $SENTRY_PID)"
    
else
    echo "❌ Error: Backend directory not found at $BACKEND_DIR"
    exit 1
fi

# 2. Wait for backend to stabilize
echo "⏳ Waiting for core services to stabilize..."
sleep 5

# 3. Start Astro Frontend
echo "🌐 Starting Mission Control Interface..."
cd "$PROJECT_ROOT"
npm run dev -- --host &
FRONTEND_PID=$!
echo "   → Frontend Interface launched (PID: $FRONTEND_PID)"

echo "============================================"
echo "✅ Operational (Server + Bridge Mode)."
echo "   Access Mission Control at: http://localhost:4321" 
echo "   (Press Ctrl+C to shutdown)"
echo "============================================"

# Wait for process exit
wait
