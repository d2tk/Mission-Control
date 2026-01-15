import http.server
import socketserver
import json
import os
from datetime import datetime

PORT = 8000
LOG_FILE = "conversation_log.json"
STATE_FILE = "mission_state.json"
DASHBOARD_FILE = "dashboard_data.json"

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
            if os.path.exists(DASHBOARD_FILE):
                with open(DASHBOARD_FILE, 'r') as f:
                    self.wfile.write(f.read().encode())
            else:
                # Default dashboard data
                default_data = {
                    "systems": [
                        { "name": "Antigravity", "status": "operational", "uptime": "99.9%" },
                        { "name": "ChatGPT", "status": "operational", "uptime": "99.8%" },
                        { "name": "Grok", "status": "operational", "uptime": "99.7%" },
                        { "name": "Browser Bridge", "status": "operational", "uptime": "100%" }
                    ],
                    "metrics": [
                        { "label": "Active Agents", "value": "4", "trend": "+0", "color": "success" },
                        { "label": "Tasks Completed", "value": "127", "trend": "+12", "color": "info" },
                        { "label": "Uptime", "value": "99.8%", "trend": "+0.1%", "color": "success" }
                    ],
                    "activities": [
                        { "time": "07:02", "agent": "Antigravity", "action": "Initialized Mission Control", "type": "info" }
                    ],
                    "projects": [
                         { "name": "Mission Control Board", "description": "Central command center for visualizing multi-agent operations.", "status": "Active", "tags": ["Visual Management"] }
                    ]
                }
                self.wfile.write(json.dumps(default_data).encode())
        else:
            super().do_GET()

    def do_POST(self):
        if self.path == '/api/messages':
            content_length = int(self.headers['Content-Length'])
            post_data = self.rfile.read(content_length)
            new_message = json.loads(post_data.decode())
            
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
            
            # Save
            with open(LOG_FILE, 'w') as f:
                json.dump(messages, f, indent=2)
                
            self.send_response(201)
            self.send_header('Content-type', 'application/json')
            self.send_header('Access-Control-Allow-Origin', '*')
            self.end_headers()
            self.wfile.write(json.dumps(new_message).encode())
        elif self.path == '/api/state':
            content_length = int(self.headers['Content-Length'])
            post_data = self.rfile.read(content_length)
            updates = json.loads(post_data.decode())
            
            state = {}
            if os.path.exists(STATE_FILE):
                with open(STATE_FILE, 'r') as f:
                    try: state = json.load(f)
                    except: state = {}
            
            # Simple merge
            state.update(updates)
            
            with open(STATE_FILE, 'w') as f:
                json.dump(state, f, indent=2)
                
            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.send_header('Access-Control-Allow-Origin', '*')
            self.end_headers()
            self.wfile.write(json.dumps(state).encode())
        elif self.path == '/api/dashboard':
            content_length = int(self.headers['Content-Length'])
            post_data = self.rfile.read(content_length)
            dashboard_data = json.loads(post_data.decode())
            
            with open(DASHBOARD_FILE, 'w') as f:
                json.dump(dashboard_data, f, indent=2)
                
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
