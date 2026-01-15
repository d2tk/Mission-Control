#!/bin/bash
# start.sh - Virtual Chat Room 통합 시작 스크립트
# Usage: ./start.sh

set -e

cd /home/a2/Desktop/gem/opb/backend
source .venv/bin/activate

echo "=== Virtual Chat Room Startup ==="
echo ""

# 1. 기존 프로세스 정리
echo "[1/4] Cleaning up existing processes..."
pkill -f "python3 server.py" 2>/dev/null || true
pkill -f "python3 server.py" 2>/dev/null || true
pkill -f "python3 browser_bridge.py" 2>/dev/null || true
pkill -f "python3 sentry.py" 2>/dev/null || true
sleep 2

# 2. Server 시작
echo "[2/4] Starting server.py..."
nohup /home/a2/Desktop/gem/opb/backend/.venv/bin/python3 server.py > server.log 2>&1 &
SERVER_PID=$!
echo "  → Server PID: $SERVER_PID"

# 3. Server 준비 대기 (포트 8000 확인)
echo "[3/4] Waiting for server to be ready..."
for i in {1..10}; do
    if curl -s http://localhost:8000 > /dev/null 2>&1; then
        echo "  → Server ready on port 8000"
        break
    fi
    if [ $i -eq 10 ]; then
        echo "  ✗ Server failed to start"
        exit 1
    fi
    sleep 1
done

# 4. Bridge 시작
echo "[4/4] Starting browser_bridge.py..."
nohup /home/a2/Desktop/gem/opb/backend/.venv/bin/python3 browser_bridge.py > bridge.log 2>&1 &
BRIDGE_PID=$!
echo "  → Bridge PID: $BRIDGE_PID"

# 5. Sentry 시작
echo "[5/6] Starting sentry.py..."
nohup /home/a2/Desktop/gem/opb/backend/.venv/bin/python3 sentry.py > sentry.log 2>&1 &
SENTRY_PID=$!
echo "  → Sentry PID: $SENTRY_PID"

sleep 3

# 6. 상태 확인
echo ""
echo "=== System Status ==="
ps aux | grep -E "server.py|browser_bridge.py|sentry.py" | grep -v grep || echo "No processes found"
echo ""
echo "=== Quick Commands ==="
echo "  View server log:  tail -f server.log"
echo "  View bridge log:  tail -f bridge.log"
echo "  Access UI:        http://localhost:8000"
echo "  Stop all:         pkill -f 'python3 server.py|browser_bridge.py'"
echo ""
echo "All systems operational. Roger. Over."
