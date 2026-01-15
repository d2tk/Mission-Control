import http.server
import socketserver
import json
import os
import subprocess
from datetime import datetime

PORT = 8000
LOG_FILE = "conversation_log.json"
STATE_FILE = "mission_state.json"
DASHBOARD_FILE = "dashboard_data.json"

# --- Concurrency Helpers ---
import fcntl
import tempfile
import shutil

class FileLock:
    """Context manager for OS-level file locking using a sidecar .lock file."""
    def __init__(self, filepath):
        self.lockfile = filepath + ".lock"
        self.fd = None

    def __enter__(self):
        self.fd = open(self.lockfile, 'w')
        # LOCK_EX: Exclusive lock (others wait)
        fcntl.flock(self.fd, fcntl.LOCK_EX)
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.fd:
            fcntl.flock(self.fd, fcntl.LOCK_UN)
            self.fd.close()

def atomic_write(filepath, data):
    """Writes data to a temp file then atomically replaces target."""
    dirpath = os.path.dirname(os.path.abspath(filepath))
    # Write to temp file
    with tempfile.NamedTemporaryFile('w', dir=dirpath, delete=False) as tmp:
        json.dump(data, tmp, indent=2)
        tmp.flush()
        os.fsync(tmp.fileno()) # Ensure write to disk
        tmp_name = tmp.name
    
    # Atomic replace
    os.replace(tmp_name, filepath)
# ---------------------------

class ChatHandler(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        if self.path == '/api/messages':
            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.send_header('Access-Control-Allow-Origin', '*')
            self.end_headers()
            if os.path.exists(LOG_FILE):
                with open(LOG_FILE, 'r') as f:
                    self.wfile.write(f.read().encode())
            else:
                self.wfile.write(b'[]')
        elif self.path == '/api/state':
            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.send_header('Access-Control-Allow-Origin', '*')
            self.end_headers()
            if os.path.exists(STATE_FILE):
                with open(STATE_FILE, 'r') as f:
                    self.wfile.write(f.read().encode())
            else:
                self.wfile.write(b'{}')
        elif self.path == '/api/dashboard':
            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.send_header('Access-Control-Allow-Origin', '*')
            self.end_headers()

            # Helper to check if a process is running using ps
            def check_process(name):
                try:
                    # Use ps aux to get full list and grep explicitly
                    # This is often more reliable than pgrep in some envs
                    output = subprocess.check_output(["ps", "aux"], text=True)
                    for line in output.splitlines():
                        if name in line and "grep" not in line:
                            return True
                    return False
                except:
                    return False

            # Check core systems
            sentry_status = check_process("sentry.py")
            bridge_status = check_process("browser_bridge.py")
            server_status = True # Self-evidently true if we are serving requests

            systems = [
                { 
                    "name": "🖥️ Server", 
                    "status": "operational" if server_status else "down"
                },
                { 
                    "name": "🌉 Browser Bridge", 
                    "status": "operational" if bridge_status else "down"
                },
                { 
                    "name": "🛡️ Sentry", 
                    "status": "operational" if sentry_status else "down"
                }
            ]

            # Logic for global status: ALL must be operational
            all_systems_go = server_status and bridge_status and sentry_status

            # Base data
            dashboard_data = {
                "global_status": "operational" if all_systems_go else "critical",
                "systems": systems,
                "activities": [
                    { "time": "07:02", "agent": "🧠 Antigravity", "action": "Initialized Mission Control", "type": "info" }
                ],
                "projects": [
                     { "name": "🎛️ Mission Control Board", "description": "Central command center for visualizing multi-agent operations.", "status": "Active", "tags": ["Visual Management"] }
                ]
            }

            # Try to merge with existing file data if available (for projects/activities)
            if os.path.exists(DASHBOARD_FILE):
                try:
                    with open(DASHBOARD_FILE, 'r') as f:
                        file_data = json.load(f)
                        if "projects" in file_data: dashboard_data["projects"] = file_data["projects"]
                        if "activities" in file_data: dashboard_data["activities"] = file_data["activities"]
                        if "metrics" in file_data: dashboard_data["metrics"] = file_data["metrics"]
                except: pass
            
            # Always override systems with real-time data
            dashboard_data["systems"] = systems
            # Inject the global flag
            dashboard_data["all_systems_go"] = all_systems_go

            self.wfile.write(json.dumps(dashboard_data).encode())
        else:
            super().do_GET()

    def do_POST(self):
        if self.path == '/api/messages':
            content_length = int(self.headers['Content-Length'])
            post_data = self.rfile.read(content_length)
            new_message = json.loads(post_data.decode())
            
            with FileLock(LOG_FILE):
                # Read existing
                messages = []
                if os.path.exists(LOG_FILE):
                    with open(LOG_FILE, 'r') as f:
                        try:
                            messages = json.load(f)
                        except:
                            messages = []
                
                # Add new
                new_message['id'] = len(messages)
                new_message['timestamp'] = datetime.now().isoformat()
                messages.append(new_message)
                
                # Atomic Save
                atomic_write(LOG_FILE, messages)
                
            self.send_response(201)
            self.send_header('Content-type', 'application/json')
            self.send_header('Access-Control-Allow-Origin', '*')
            self.end_headers()
            self.wfile.write(json.dumps(new_message).encode())

        elif self.path == '/api/state':
            content_length = int(self.headers['Content-Length'])
            post_data = self.rfile.read(content_length)
            updates = json.loads(post_data.decode())
            
            with FileLock(STATE_FILE):
                state = {}
                if os.path.exists(STATE_FILE):
                    with open(STATE_FILE, 'r') as f:
                        try: state = json.load(f)
                        except: state = {}
                
                # Simple merge
                for key, value in updates.items():
                    if isinstance(value, dict) and key in state and isinstance(state[key], dict):
                        state[key].update(value)
                    else:
                        state[key] = value
                
                atomic_write(STATE_FILE, state)

            # --- AUTO-LOG ACTIVITY ---
            # Separate lock for dashboard data to avoid deadlocks overlapping with state lock if not needed
            # But here we are just appending, so a lock is good.
            try:
                with FileLock(DASHBOARD_FILE):
                    # Load existing dashboard data
                    dash_data = {}
                    if os.path.exists(DASHBOARD_FILE):
                        with open(DASHBOARD_FILE, 'r') as f:
                            try: dash_data = json.load(f)
                            except: pass
                    
                    if "activities" not in dash_data:
                        dash_data["activities"] = []

                    new_activities = []
                    timestamp = datetime.now().strftime("%H:%M")

                    # Check for agent updates
                    if "agents" in updates:
                        for agent_name, agent_data in updates["agents"].items():
                            # Logic for human-readable activity logs
                            status = agent_data.get("status")
                            current_task = agent_data.get("current_task")
                            last_task = agent_data.get("last_task")
                            
                            action_text = ""
                            msg_type = "info"

                            if status == "busy":
                                if current_task:
                                    task_summary = (current_task[:50] + '...') if len(current_task) > 50 else current_task
                                    action_text = f"Started working on: {task_summary}"
                                else:
                                    action_text = "Is now busy working on a task."
                            
                            elif status == "idle":
                                if last_task:
                                    task_summary = (last_task[:50] + '...') if len(last_task) > 50 else last_task
                                    action_text = f"Completed task: {task_summary}"
                                    msg_type = "success"
                                else:
                                    action_text = "Completed task and is standing by."
                                    msg_type = "success"
                            
                            elif status: # unexpected status
                                action_text = f"Status changed to: {status}"

                            if action_text:
                                new_activities.append({
                                    "time": timestamp,
                                    "agent": agent_name,
                                    "action": action_text,
                                    "type": msg_type
                                })
                    
                    # Check for mission status updates
                    if "status" in updates:
                         new_activities.append({
                            "time": timestamp,
                            "agent": "Mission Control",
                            "action": f"Mission status updated to: {updates['status']}",
                            "type": "warning"
                        })

                    if new_activities:
                        # Prepend new activities
                        dash_data["activities"] = new_activities + dash_data["activities"]
                        # Limit to last 50
                        dash_data["activities"] = dash_data["activities"][:50]
                        
                        atomic_write(DASHBOARD_FILE, dash_data)
            except Exception as e:
                print(f"Error logging activity: {e}")
            # -------------------------
                
            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.send_header('Access-Control-Allow-Origin', '*')
            self.end_headers()
            self.wfile.write(json.dumps(state).encode())

        elif self.path == '/api/dashboard':
            content_length = int(self.headers['Content-Length'])
            post_data = self.rfile.read(content_length)
            dashboard_data = json.loads(post_data.decode())
            
            with FileLock(DASHBOARD_FILE):
                atomic_write(DASHBOARD_FILE, dashboard_data)
                
            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.send_header('Access-Control-Allow-Origin', '*')
            self.end_headers()
            self.wfile.write(json.dumps({'status': 'ok'}).encode())

    def do_OPTIONS(self):
        self.send_response(200)
        self.send_header('Access-Control-Allow-Origin', '*')
        self.send_header('Access-Control-Allow-Methods', 'GET, POST, OPTIONS')
        self.send_header('Access-Control-Allow-Headers', 'Content-Type')
        self.end_headers()

class ReusableTCPServer(socketserver.ThreadingTCPServer):
    allow_reuse_address = True

print(f"Serving Chat Room at http://localhost:{PORT}")
with ReusableTCPServer(("", PORT), ChatHandler) as httpd:
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        pass
