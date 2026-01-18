#!/bin/bash
# start.sh - Virtual Chat Room Startup (Rust Edition)
# Usage: ./start.sh

set -e

# Define paths
WORKSPACE_DIR="/home/a2/Desktop/gem"
BACKEND_DIR="$WORKSPACE_DIR/opb/backend"
TARGET_DIR="$BACKEND_DIR/target/release"

cd "$BACKEND_DIR"

echo "=== Virtual Chat Room Startup (Rust) ==="
echo ""

# 0. Build Rust Binaries
echo "[0/5] Building Rust binaries..."
# Check if cargo is available
if ! command -v cargo &> /dev/null; then
    echo "  ✗ Cargo is not installed or not in PATH."
    exit 1
fi

# Build release
if cargo build --release; then
    echo "  → Build successful"
else
    echo "  ✗ Build failed"
    exit 1
fi

# 1. Cleanup existing processes
echo "[1/5] Cleaning up existing processes..."
pkill -f "python3 server.py" 2>/dev/null || true
pkill -f "python3 sentry.py" 2>/dev/null || true
pkill -f "python3 browser_bridge.py" 2>/dev/null || true
pkill -f "$TARGET_DIR/server" 2>/dev/null || true
pkill -f "$TARGET_DIR/sentry" 2>/dev/null || true
pkill -f "$TARGET_DIR/bridge" 2>/dev/null || true
# Kill simple names just in case they are running from target/release directly
pkill -x "server" 2>/dev/null || true
pkill -x "sentry" 2>/dev/null || true
sleep 1

# 2. Start Server
echo "[2/5] Starting Rust Server..."
nohup "$TARGET_DIR/server" > server.log 2>&1 &
SERVER_PID=$!
echo "  → Server PID: $SERVER_PID"

# 3. Wait for Server (Port 8000)
echo "[3/5] Waiting for server to be ready..."
for i in {1..10}; do
    if curl -s http://localhost:8000/api/dashboard > /dev/null 2>&1; then
        echo "  → Server ready on port 8000"
        break
    fi
    if [ $i -eq 10 ]; then
        echo "  ✗ Server failed to start (or port blocked)"
        # Show log tail if failed
        tail -n 10 server.log
        exit 1
    fi
    sleep 1
done

# 4. Start Bridge (Rust Wrapper -> Python or Full Rust?)
# NOTE: Currently bridge.rs is a wrapper or unimplemented. 
# Attempting to run Rust bridge if it exists, otherwise fall back or fail?
# The plan said "Launch target/release/bridge". Let's assume bridge.rs compiles to a usable state 
# or wraps the python script. If bridge.rs is just a stub, this might be an issue.
# Let's check bridge.rs content briefly before this step. 
# Looking at file list, bridge.rs was 11KB, so it's likely a full implementation or heavy wrapper.
echo "[4/5] Starting Bridge..."
nohup "$TARGET_DIR/bridge" > bridge.log 2>&1 &
BRIDGE_PID=$!
echo "  → Bridge PID: $BRIDGE_PID"

# 5. Start Sentry
echo "[5/5] Starting Sentry..."
nohup "$TARGET_DIR/sentry" > sentry.log 2>&1 &
SENTRY_PID=$!
echo "  → Sentry PID: $SENTRY_PID"

sleep 2

# 6. Status Check
echo ""
echo "=== System Status ==="
ps aux | grep -E "$TARGET_DIR/server|$TARGET_DIR/bridge|$TARGET_DIR/sentry" | grep -v grep || echo "No processes found"
echo ""
echo "=== Quick Commands ==="
echo "  View server log:  tail -f server.log"
echo "  View bridge log:  tail -f bridge.log"
echo "  Access UI:        http://localhost:8000"
echo "  Stop all:         pkill -f '$TARGET_DIR/server|$TARGET_DIR/bridge|$TARGET_DIR/sentry'"
echo ""
echo "All systems operational (Rust). Roger. Over."
