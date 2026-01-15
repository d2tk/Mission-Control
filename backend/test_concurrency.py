import concurrent.futures
import urllib.request
import json
import time
import random

API_URL = "http://localhost:8000/api/state"
CONCURRENT_REQUESTS = 20

def send_update(i):
    # Each request updates a unique key to allow verification
    payload = {
        "agents": {
            f"Agent_{i}": {
                "status": "active",
                "last_task": f"Task_{i}"
            }
        },
        "status": "stress_testing"
    }
    
    try:
        data = json.dumps(payload).encode('utf-8')
        req = urllib.request.Request(API_URL, data=data, headers={'Content-Type': 'application/json'})
        with urllib.request.urlopen(req, timeout=5) as f:
            if f.status == 200:
                return True
    except Exception as e:
        print(f"Request {i} failed: {e}")
        return False
    return False

def verify_state(total_requests):
    req = urllib.request.Request(API_URL)
    with urllib.request.urlopen(req) as f:
        state = json.loads(f.read().decode('utf-8'))
        
    agents = state.get("agents", {})
    count = 0
    for i in range(total_requests):
        if f"Agent_{i}" in agents:
            count += 1
            
    print(f"Verification: Found {count}/{total_requests} agent updates.")
    return count == total_requests

if __name__ == "__main__":
    print(f"Starting stress test with {CONCURRENT_REQUESTS} concurrent requests...")
    start_time = time.time()
    
    with concurrent.futures.ThreadPoolExecutor(max_workers=10) as executor:
        results = list(executor.map(send_update, range(CONCURRENT_REQUESTS)))
        
    duration = time.time() - start_time
    print(f"Completed in {duration:.2f} seconds.")
    
    success_count = sum(results)
    print(f"Successful requests: {success_count}/{CONCURRENT_REQUESTS}")
    
    if verify_state(CONCURRENT_REQUESTS):
        print("TEST PASSED: No lost updates detected.")
    else:
        print("TEST FAILED: Some updates were lost!")
