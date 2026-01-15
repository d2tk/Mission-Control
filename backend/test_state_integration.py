import urllib.request
import urllib.parse
import json
import time

STATE_URL = "http://localhost:8000/api/state"
DASHBOARD_URL = "http://localhost:8000/api/dashboard"

def test_integration():
    print("1. Sending state update...")
    update_payload = {
        "agents": {
            "TestAgent": {
                "status": "active_test",
                "last_task": "Running verification2"
            }
        },
        "status": "testing2"
    }
    
    try:
        data = json.dumps(update_payload).encode('utf-8')
        req = urllib.request.Request(STATE_URL, data=data, headers={'Content-Type': 'application/json'})
        with urllib.request.urlopen(req) as f:
            print(f"Update response: {f.status}")
    except Exception as e:
        print(f"Failed to connect to server: {e}")
        return

    print("2. Verifying dashboard data...")
    try:
        with urllib.request.urlopen(DASHBOARD_URL) as f:
            data = json.loads(f.read().decode('utf-8'))
        
        activities = data.get("activities", [])
        found_agent = False
        found_status = False
        
        for act in activities:
            if act.get("agent") == "TestAgent" and "active_test" in act.get("action", ""):
                found_agent = True
            if act.get("agent") == "Mission Control" and "testing2" in act.get("action", ""):
                found_status = True
                
        if found_agent:
            print("[PASS] Found agent status update log.")
        else:
            print("[FAIL] Agent status update NOT found.")
            
        if found_status:
            print("[PASS] Found mission status update log.")
        else:
            print("[FAIL] Mission status update NOT found.")
            
        print("\nRecent Activities dump:")
        for act in activities[:3]:
            print(act)

    except Exception as e:
        print(f"Failed to fetch dashboard: {e}")

if __name__ == "__main__":
    test_integration()
