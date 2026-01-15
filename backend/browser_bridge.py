import asyncio
import json
import urllib.request
import urllib.parse
import os
import time
import subprocess
from pathlib import Path
from playwright.async_api import async_playwright

API_URL = "http://localhost:8000/api/messages"
STATE_URL = "http://localhost:8000/api/state"
POLL_INTERVAL = 2
USER_DATA_DIR = "./browser_data"
PROCESSED_FILE = "processed_ids.txt"

class AIBridge:
    def __init__(self):
        self.last_message_id = -1
        self.processed_ids = self.load_processed_ids()
        self.busy_agents = set()
        self.pages = {}
        self.agent_config = {
            'chatgpt': {'url': 'https://chatgpt.com/', 'lifecycle': 'warm'}
        }
        self.playwright = None
        self.context = None

    def load_processed_ids(self):
        if os.path.exists(PROCESSED_FILE):
            try:
                with open(PROCESSED_FILE, 'r') as f:
                    return set(int(line.strip()) for line in f if line.strip())
            except: return set()
        return set()

    def save_processed_id(self, msg_id):
        self.processed_ids.add(msg_id)
        try:
            with open(PROCESSED_FILE, 'a') as f:
                f.write(f"{msg_id}\n")
        except: pass

    async def start(self):
        print("Starting AI Bridge (Lifecycle Edition)...", flush=True)
        self.playwright = await async_playwright().start()
        self.context = await self.playwright.chromium.launch_persistent_context(
            user_data_dir=USER_DATA_DIR,
            headless=False,
            viewport=None,
            args=["--disable-blink-features=AutomationControlled", "--no-sandbox", "--disable-infobars"]
        )
        
        # Initial boot for 'warm' agents
        for name, cfg in self.agent_config.items():
            if cfg['lifecycle'] == 'warm':
                await self.ensure_tab(name)

        await self.sync_initial_state()
        print("\n=== Bridge Ready (Lifecycle Managed) ===", flush=True)
        
        while True:
            await self.check_messages()
            await asyncio.sleep(POLL_INTERVAL)

    async def ensure_tab(self, name):
        if name in self.pages and not self.pages[name].is_closed():
            return self.pages[name]
        
        print(f"Opening tab for {name}...")
        cfg = self.agent_config.get(name)
        if not cfg: return None
        
        page = await self.context.new_page()
        await page.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined})")
        try:
            await page.goto(cfg['url'], wait_until="domcontentloaded", timeout=60000)
            self.pages[name] = page
            print(f"Tab for {name} ready.")
            return page
        except Exception as e:
            print(f"Error opening {name}: {e}")
            await page.close()
            return None

    async def sync_initial_state(self):
        try:
            req = urllib.request.Request(API_URL)
            with urllib.request.urlopen(req, timeout=5) as response:
                if response.status == 200:
                    messages = json.loads(response.read().decode())
                    if messages:
                        self.last_message_id = messages[-1]['id']
        except Exception as e:
            print(f"Error syncing state: {e}")

    async def check_messages(self):
        try:
            req = urllib.request.Request(API_URL)
            res_data = await asyncio.to_thread(lambda: urllib.request.urlopen(req, timeout=5).read())
            messages = json.loads(res_data.decode())
            
            for msg in messages:
                msg_id = msg['id']
                if msg_id in self.processed_ids: continue
                if msg_id > self.last_message_id: self.last_message_id = msg_id

                sender = msg.get('sender', '')
                text = msg.get('message', '')
                lower_text = text.lower()
                
                if "!brief" in text:
                    print("Command: !brief detected. Initiating Injection Protocol...", flush=True)
                    asyncio.create_task(self.inject_briefings(msg_id))
                    self.save_processed_id(msg_id)
                    continue

                if "!audit" in text:
                    print("Command: !audit detected. Running Project Sentry...", flush=True)
                    asyncio.create_task(self.run_audit(msg_id))
                    self.save_processed_id(msg_id)
                    continue

                # Structured Task Trigger (JSON Envelope)
                if text.strip().startswith('{') and text.strip().endswith('}'):
                    try:
                        envelope = json.loads(text)
                        target_agent = envelope.get('assigned_to')
                        if target_agent and target_agent.lower() in self.agent_config:
                            print(f"Mission Envelope detected for {target_agent}. Dispatching...", flush=True)
                            prompt = json.dumps(envelope.get('input', {}))
                            await self.trigger_agent(target_agent, target_agent.lower(), prompt, "", msg_id)
                            self.save_processed_id(msg_id)
                            continue
                    except: pass

                # Role Triggers (Legacy @mentions)
                if "@chatgpt" in lower_text and sender != "ChatGPT":
                    await self.trigger_agent('ChatGPT', 'chatgpt', text, "@chatgpt", msg_id)
                if "@grok" in lower_text and sender != "Grok":
                    await self.trigger_agent('Grok', 'grok', text, "@grok", msg_id)
                if ("@antigravity" in lower_text or "@gemini" in lower_text) and sender != "Antigravity":
                    trigger = "@antigravity" if "@antigravity" in lower_text else "@gemini"
                    await self.trigger_agent('Antigravity', 'gemini', text, trigger, msg_id)

        except: pass

    async def inject_briefings(self, msg_id):
        briefings = {
            'ChatGPT': ('chatgpt', 'briefings/tactical_brief.md')
        }
        
        for agent_name, (page_key, file_path) in briefings.items():
            if self.agent_config[page_key]['lifecycle'] != 'warm':
                continue
                
            try:
                with open(file_path, 'r') as f:
                    content = f.read()
                
                print(f"Injecting briefing for {agent_name}...", flush=True)
                
                # Re-use existing handler logic but with briefing content
                if agent_name == 'ChatGPT':
                    await self.handle_generic(agent_name, page_key, content, 'div#prompt-textarea', self.chatgpt_scraper)
                    
            except Exception as e:
                print(f"Failed to inject briefing for {agent_name}: {e}", flush=True)

    async def run_audit(self, msg_id):
        """Execute Project Sentry audit and post report"""
        try:
            print("Executing sentry.py...", flush=True)
            result = await asyncio.to_thread(
                subprocess.run,
                ['python3', 'sentry.py', '--once'],
                cwd='/home/a2/Desktop/gem/opb/backend',
                capture_output=True,
                text=True,
                timeout=30
            )
            
            if result.returncode == 0:
                report = result.stdout
                await self.post_message("Antigravity", f"**PROJECT SENTRY REPORT**\n\n{report}")
                print("Audit report posted successfully.", flush=True)
            else:
                error_msg = f"Sentry audit failed: {result.stderr}"
                await self.post_message("Antigravity", error_msg)
                print(error_msg, flush=True)
                
        except Exception as e:
            error_msg = f"Audit execution error: {str(e)}"
            await self.post_message("Antigravity", error_msg)
            print(error_msg, flush=True)

    async def trigger_agent(self, agent_name, page_key, text, trigger, msg_id):
        if agent_name in self.busy_agents:
            print(f"Block: {agent_name} is busy.")
            return


        prompt = self.extract_prompt(text, trigger)
        self.busy_agents.add(agent_name)
        self.save_processed_id(msg_id)
        
        # Handle the task based on role
        if agent_name == 'ChatGPT':
            asyncio.create_task(self.handle_generic(agent_name, page_key, prompt, 'div#prompt-textarea', self.chatgpt_scraper))
        elif agent_name == 'Grok':
            asyncio.create_task(self.handle_generic(agent_name, page_key, prompt, 'textarea[aria-label="Ask Grok anything"]', self.grok_scraper))
        elif agent_name == 'Antigravity':
            asyncio.create_task(self.handle_generic(agent_name, page_key, prompt, 'div.ql-editor', self.gemini_scraper))

    def extract_prompt(self, text, trigger):
        idx = text.lower().find(trigger)
        return text[idx + len(trigger):].strip()

    async def handle_generic(self, agent_name, page_key, prompt, selector, scraper):
        try:
            await self.post_message(agent_name, "Thinking... Roger.")
            await self.update_state({"agents": {agent_name: {"status": "busy", "current_task": prompt[:50] + "..." if len(prompt) > 50 else prompt}}})
            
            page = await self.ensure_tab(page_key)
            if not page: raise Exception("Failed to ensure tab.")
            
            await page.bring_to_front()
            
            # Input
            target = page.locator(selector).first
            await target.wait_for(state='visible', timeout=15000)
            await target.click()
            await target.fill(prompt) 
            await page.keyboard.press('Enter')
            
            # Event-Driven Wait (MutationObserver)
            try:
                await self.wait_for_completion(page)
            except Exception as e:
                print(f"Wait warning: {e}", flush=True)
                # Fallback small sleep if observer fails
                await asyncio.sleep(5)
            
            # Scrape specific latest message
            answer = await scraper(page)
            
            if not answer:
                answer = "Extraction failed. No new message detected. Over. Roger."
            
            await self.post_message(agent_name, answer)
            await self.update_state({"agents": {agent_name: {"status": "idle", "last_task": prompt[:50]}}})
            
            # Lifecycle: Close non-warm tabs after use
            if self.agent_config[page_key]['lifecycle'] != 'warm':
                print(f"Closing non-warm tab for {agent_name}")
                await page.close()
                del self.pages[page_key]

        except Exception as e:
            await self.post_message(agent_name, f"Error: {str(e)}. Over. Roger.")
        finally:
            if agent_name in self.busy_agents: self.busy_agents.remove(agent_name)

    async def wait_for_completion(self, page):
        """Waits for DOM mutations to settle (silence for 1.5s)."""
        print("Waiting for generation to settle...", flush=True)
        await page.evaluate("""
            window.__waitForAssistant = () => new Promise(resolve => {
                let lastChange = Date.now();
                const obs = new MutationObserver(() => { lastChange = Date.now(); });
                // Observe entire body for changes (text-streaming usually triggers this)
                obs.observe(document.body, { childList: true, subtree: true, characterData: true });
                
                const check = setInterval(() => {
                    const silence = Date.now() - lastChange;
                    // If 1.5s of silence, we assume generation stopped. 
                    // Verify also that Stop button is gone if possible (optional heuristic improvement)
                    if (silence > 1500) {
                        clearInterval(check);
                        obs.disconnect();
                        resolve(true);
                    }
                }, 300);
                
                // Safety timeout 60s
                setTimeout(() => {
                     clearInterval(check);
                     obs.disconnect();
                     resolve(true);
                }, 60000);
            });
        """)
        await page.evaluate("window.__waitForAssistant()")
        print("Generation settled.", flush=True)

    async def chatgpt_scraper(self, page):
        try:
            return await page.evaluate("""() => {
                const msgs = [...document.querySelectorAll('div[data-message-author-role="assistant"]')];
                if (msgs.length === 0) return null;
                const last = msgs[msgs.length - 1];
                return last.innerText;
            }""")
        except Exception as e:
            print(f"ChatGPT Scraper Error: {e}")
            return None

    async def gemini_scraper(self, page):
        try:
            return await page.evaluate("""() => {
                 const msgs = [...document.querySelectorAll('div.message-content')];
                 if (msgs.length === 0) return null;
                 return msgs[msgs.length - 1].innerText;
            }""")
        except: return None

    async def grok_scraper(self, page):
        try:
            return await page.evaluate("""() => {
                const msgs = [...document.querySelectorAll('div.message-row-assistant, .markdown.prose')];
                if (msgs.length === 0) return null;
                return msgs[msgs.length - 1].innerText;
            }""")
        except Exception as e:
            print(f"Grok Scraper Error: {e}")
            return None

    async def generic_scraper(self, page):
        # Fallback
        return None

    async def post_message(self, sender, text):
        try: 
            data = json.dumps({"sender": sender, "message": text}).encode('utf-8')
            req = urllib.request.Request(API_URL, data=data, headers={'Content-Type': 'application/json'})
            await asyncio.to_thread(lambda: urllib.request.urlopen(req, timeout=5))
        except: pass

    async def update_state(self, updates):
        try: 
            data = json.dumps(updates).encode('utf-8')
            req = urllib.request.Request(STATE_URL, data=data, headers={'Content-Type': 'application/json'})
            await asyncio.to_thread(lambda: urllib.request.urlopen(req, timeout=5))
        except: pass

if __name__ == "__main__":
    bridge = AIBridge()
    asyncio.run(bridge.start())
