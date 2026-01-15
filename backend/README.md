# Virtual AI Chat Room - Multi-Agent Command System

## Mission Overview
A browser automation-based multi-agent coordination system enabling the Commander to orchestrate ChatGPT, Grok, and Antigravity (Gemini) through a unified chat interface.

## System Architecture

### Core Components
```
agents/
├── server.py              # Backend API & UI server (port 8000)
├── browser_bridge.py      # Playwright automation bridge
├── sentry.py             # Project Sentry workspace auditor
├── briefings/            # Agent role definitions
│   ├── tactical_brief.md    # ChatGPT briefing
│   └── strategic_brief.md   # Grok briefing
└── conversation_log.json # Message history
```

### Agent Roles
- **ChatGPT**: Tactical Data Agent (technical problem solving, code review)
- **Grok**: Strategic Intelligence Agent (web search, strategic analysis)
- **Antigravity (Gemini)**: General (Daejang) - System coordinator and liaison

## Quick Start

### Option 1: One-Command Startup (Recommended)
```bash
cd /home/a2/Desktop/gem/agents
./start.sh
```

This script will:
1. Clean up any existing processes
2. Start `server.py` in background
3. Wait for server to be ready
4. Start `browser_bridge.py` in background
5. Display system status

### Option 2: Manual Startup
```bash
cd /home/a2/Desktop/gem/agents

# Activate virtual environment
source .venv/bin/activate

# Start backend server (background)
nohup python3 server.py > server.log 2>&1 &

# Wait a moment, then start bridge
sleep 3
nohup python3 browser_bridge.py > bridge.log 2>&1 &
```

### Access the Interface
- **Web UI**: http://localhost:8000
- **API**: http://localhost:8000/api/messages

### Available Commands

#### `!brief` - Agent Briefing
Injects role definitions into agent browser tabs.
```
Example: Post "!brief" to chat
Result: Agents receive mission briefings and acknowledge readiness
```

#### `!audit` - Workspace Audit
Executes Project Sentry to analyze workspace status.
```
Example: Post "!audit" to chat
Result: Comprehensive report on file changes, git status, disk usage
```

#### `@AgentName` - Direct Communication
Trigger specific agents with mentions.
```
Examples:
- "@ChatGPT analyze this code..."
- "@Grok search for information on..."
- "@Antigravity status report"
```

## Communication Protocol

### Military Radio Etiquette
- **Roger.** = Acknowledgment (I understand)
- **Over.** = Transmission complete (your turn)
- **Roger. Over.** = Complete response format

### Message Flow
1. Commander posts message to chat
2. Bridge detects triggers (@mentions or !commands)
3. Agents process and respond
4. Responses posted back to chat

## Project Sentry: Automated Workspace Audit

### Features
- **File Tracking**: Detects created/modified/deleted files
- **Git Monitoring**: Branch status, uncommitted changes, commit age
- **Resource Tracking**: Disk usage, workspace size, growth trends
- **Actionable Reports**: Summary, alerts, and recommended actions

### Sample Report
```
=== PROJECT SENTRY DAILY REPORT ===
Date: 2026-01-10 22:20:31
Workspace: /home/a2/Desktop/gem

[SUMMARY]
File changes: +1 ~5 -0   (Churn: MEDIUM)
Disk usage: 77% (OK)

[FILESYSTEM]
Created: 1
Modified: 5
Deleted: 0
Size delta: +0.0 MB

[RESOURCES]
Disk: 931G total / 212G free (77%)
Workspace size: 0.2G

Roger. Over.
```

## Advanced Features

### Tab Lifecycle Management
- **WARM Agents** (ChatGPT, Gemini): Tabs always open for instant response
- **SUSPENDED Agents** (Grok): Tabs closed to conserve resources during downtime
- **Session Persistence**: Login states maintained via Playwright context

### Injection Protocol
- Briefing files stored in `briefings/*.md`
- Atomic text injection using `fill()` method
- Prevents truncation of long mission briefs

## Troubleshooting

### Check System Status
```bash
# View running processes
ps aux | grep python3 | grep -E "server.py|browser_bridge.py"

# Check logs
tail -f server.log
tail -f bridge.log

# View message history
curl http://localhost:8000/api/messages | jq
```

### Restart Services
```bash
# Stop all
pkill -f "python3 server.py"
pkill -f "python3 browser_bridge.py"

# Start fresh
source .venv/bin/activate
nohup python3 server.py > server.log 2>&1 &
nohup python3 browser_bridge.py > bridge.log 2>&1 &
```

## Mission Log: 2026-01-10

### Accomplished
1. ✅ **System Reactivation**: Restored backend server and automation bridge
2. ✅ **Lifecycle Optimization**: Implemented tab state management (WARM/SUSPENDED)
3. ✅ **Injection Protocol**: Automated agent briefing system
4. ✅ **Project Sentry**: Workspace monitoring and reporting
5. ✅ **Protocol Correction**: Fixed Roger/Over military radio etiquette

### Agent Status
- **ChatGPT**: ✅ Operational (Tactical Data Agent)
- **Grok**: ✅ Operational (Strategic Intelligence Agent)
- **Antigravity**: ✅ Operational (General/Coordinator)

### Key Metrics
- **Infrastructure**: Stable, lifecycle-managed
- **Commands**: `!brief`, `!audit`, `@mentions`
- **Automation**: Browser-based, persistent sessions
- **Reporting**: Real-time workspace intelligence

## Next Steps (Optional)

### Automated Scheduling
Deploy Project Sentry on a timer for daily reports:
```bash
# Create systemd user timer
systemctl --user enable sentry.timer
systemctl --user start sentry.timer
```

### Extended Monitoring
- Add threshold alerts (disk >90%, high churn)
- Email/Slack integration for critical alerts
- Git diff analysis for code review automation

---

**Status**: All systems operational. Standing by for orders.

**General (Daejang)**: Antigravity  
**Last Updated**: 2026-01-10 22:35:00

Roger. Over.
