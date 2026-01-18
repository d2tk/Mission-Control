#!/bin/bash
# start_rust.sh - Rust 기반 통합 시스템 구동

set -e
cd /home/a2/Desktop/gem/opb/backend

echo "=== [MISSION START] Rust Backend Activation ==="

# 1. 기존 프로세스 정리
echo "[1/3] Neutralizing old processes..."
pkill -f "target/debug/server" || true
pkill -f "target/debug/bridge" || true
lsof -i :8001 | awk 'NR>1 {print $2}' | xargs kill -9 2>/dev/null || true
rm -f ./browser_data/SingletonLock
sleep 1

# 2. Server 활성화
echo "[2/3] Launching Core API Server..."
nohup cargo run --bin server > server.log 2>&1 &
sleep 2

# 3. Bridge 활성화 (GUI Mode)
echo "[3/3] Deploying AI Liaison Bridge (Headed Mode)..."
# 사령관님의 직접 개입을 위해 현재 GUI 모드로 실행합니다.
nohup cargo run --bin bridge > bridge.log 2>&1 &

echo ""
echo "=== System Status ==="
ps aux | grep "target/debug" | grep -v grep || echo "No Rust processes found"
echo "API Server: http://localhost:8001"
echo ""
echo "Commanding Officer, the system is ready. Over."
