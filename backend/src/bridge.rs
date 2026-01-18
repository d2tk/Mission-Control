use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;
use serde::{Deserialize, Serialize};
use chromiumoxide::{Browser, BrowserConfig, Page, Element};
use chromiumoxide::browser::HeadlessMode;
use chromiumoxide::handler::Handler;
use chromiumoxide::layout::Point;
use chromiumoxide::cdp::browser_protocol::input::{DispatchKeyEventParams, DispatchKeyEventType}; // Low-level Input API
use serde_json::Value;
use futures::StreamExt;
use reqwest::Client;
use crate::models::{Message};

const API_BASE: &str = "http://localhost:8000/api";
const POLL_INTERVAL: u64 = 2;

type BridgeResult<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
const USER_DATA_DIR: &str = "./isolated_data";
const PROCESSED_FILE: &str = "processed_ids.txt";
// STEALTH_JS removed to fix warning as stealth is disabled

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AgentConfig {
    url: String,
    lifecycle: String,
    input_selector: String,
    output_selector: String,
}

pub struct AIBridge {
    last_message_id: usize,
    processed_ids: HashSet<usize>,
    busy_agents: HashSet<String>,
    agent_configs: HashMap<String, AgentConfig>,
    client: Client,
    active_browsers: Arc<Mutex<HashMap<String, Arc<Browser>>>>,
}

impl AIBridge {
    pub fn new() -> Self {
        let mut agent_configs = HashMap::new();
        
        agent_configs.insert("chatgpt".to_string(), AgentConfig {
            url: "https://chatgpt.com/".to_string(),
            lifecycle: "warm".to_string(),
            input_selector: "div#prompt-textarea".to_string(),
            output_selector: ".markdown, .prose, div[data-message-author-role=\"assistant\"], [data-testid^=\"conversation-turn-\"]".to_string(),
        });

        agent_configs.insert("claude".to_string(), AgentConfig {
            url: "https://claude.ai".to_string(),
            lifecycle: "warm".to_string(),
            input_selector: "div[contenteditable=\"true\"], [aria-label=\"Write your message to Claude\"], .ProseMirror".to_string(),
            output_selector: ".font-claude-message, [data-testid=\"assistant-message\"], .prose, .message-content, [data-message-author-role=\"assistant\"]".to_string(),
        });

        Self {
            last_message_id: 0,
            processed_ids: HashSet::new(),
            busy_agents: HashSet::new(),
            agent_configs,
            client: Client::new(),
            active_browsers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn load_processed_ids(&mut self) {
        if let Ok(content) = fs::read_to_string(PROCESSED_FILE) {
            self.processed_ids = content.lines()
                .filter_map(|l| l.parse::<usize>().ok())
                .collect();
        }
    }

    pub fn save_processed_id(&mut self, id: usize) {
        self.processed_ids.insert(id);
        if let Ok(mut file) = fs::OpenOptions::new().append(true).create(true).open(PROCESSED_FILE) {
            let _ = writeln!(file, "{}", id);
        }
    }

    pub async fn start(&mut self) -> BridgeResult<()> {
        println!("Starting AI Bridge (Rust On-Demand Edition)...");
        self.load_processed_ids();

        self.sync_initial_state().await?;
        println!("\n=== Bridge Ready (Rust On-Demand) ===");

        loop {
            if let Err(e) = self.check_messages().await {
                eprintln!("Error checking messages: {}", e);
            }
            sleep(Duration::from_secs(POLL_INTERVAL)).await;
        }
    }



    async fn sync_initial_state(&mut self) -> BridgeResult<()> {
        let mut retries = 30; // 60 seconds total
        let mut wait_secs = 2;
        
        while retries > 0 {
            match self.client.get(format!("{}/messages", API_BASE)).send().await {
                Ok(resp) => {
                    if let Ok(messages) = resp.json::<Vec<Message>>().await {
                        for msg in messages {
                            if let Some(id) = msg.id {
                                if id > self.last_message_id {
                                    self.last_message_id = id;
                                }
                            }
                        }
                        return Ok(());
                    }
                }
                Err(_) => {
                    retries -= 1;
                    if retries == 0 { 
                        return Err("API server failed to stand up within timeout.".into()); 
                    }
                    println!("   [System] Waiting for Mission Control Server... ({} retries left)", retries);
                    sleep(Duration::from_secs(wait_secs)).await;
                    // Cap wait time at 5 seconds
                    if wait_secs < 5 { wait_secs += 1; }
                }
            }
        }
        Ok(())
    }

    async fn check_messages(&mut self) -> BridgeResult<()> {
        let resp = self.client.get(format!("{}/messages", API_BASE)).send().await?;
        let messages: Vec<Message> = resp.json::<Vec<Message>>().await?;
        
        if !messages.is_empty() {
             // println!("Polled {} messages", messages.len());
        }

        for msg in messages {
            let msg_id = match msg.id {
                Some(id) => id,
                None => continue,
            };

            if self.processed_ids.contains(&msg_id) { continue; }
            if msg_id > self.last_message_id { self.last_message_id = msg_id; }

            let text = msg.message.trim();
            let lower_text = text.to_lowercase();
            
            // println!("Processing message {}: {}", msg_id, text);

            // Handle !brief command
            if text.contains("!brief") {
                println!("Command: !brief detected. Initiating Injection Protocol...");
                let bridge_clone = self.clone_for_task();
                tokio::spawn(async move {
                    let _ = bridge_clone.inject_briefings(msg_id).await;
                });
                self.save_processed_id(msg_id);
                continue;
            }

            // Handle !audit command
            if text.contains("!audit") {
                println!("Command: !audit detected.");
                tokio::spawn(async move {
                   let _ = AIBridge::run_audit_command().await;
                });
                self.save_processed_id(msg_id);
                continue;
            }

            // Handle JSON envelope (structured task assignment)
            if text.trim().starts_with('{') && text.trim().ends_with('}') {
                println!("Potential JSON envelope detection (ID: {})", msg_id);
                match serde_json::from_str::<Value>(text) {
                    Ok(envelope) => {
                        println!("Parsed JSON: {:?}", envelope);
                        if let Some(target) = envelope.get("assigned_to").and_then(|v| v.as_str()) {
                            let target_owned = target.to_string();
                            let target_lower = target_owned.to_lowercase();
                            if self.agent_configs.contains_key(&target_lower) {
                                println!("Agent {} found in config. Dispatching...", target_owned);
                                let input = envelope.get("input").unwrap_or(&Value::Null);
                                let prompt = if input.is_string() { 
                                    input.as_str().unwrap().to_string() 
                                } else { 
                                    serde_json::to_string(input).unwrap_or_default() 
                                };
                                
                                // Spawn non-blocking task
                                let mut bridge_clone = self.clone_for_task();
                                let prompt_clone = prompt.clone();
                                let target_clone = target_owned.clone();
                                let target_lower_clone = target_lower.clone();
                                
                                tokio::spawn(async move {
                                    let _ = bridge_clone.handle_agent_trigger(&target_clone, &target_lower_clone, &prompt_clone, "", msg_id).await;
                                });
                                
                                self.save_processed_id(msg_id);
                                continue;
                            }
                        }
                    },
                    Err(e) => {
                        eprintln!("Failed to parse potential JSON envelope (ID: {}): {}", msg_id, e);
                        // Save anyway to avoid looping on malformed JSON
                        self.save_processed_id(msg_id);
                        continue;
                    }
                }
            }

            // Handle @mentions
            if lower_text.contains("@chatgpt") && msg.sender != "ChatGPT" {
                let mut bridge_clone = self.clone_for_task();
                let text_clone = text.to_string();
                tokio::spawn(async move {
                    let _ = bridge_clone.handle_agent_trigger("ChatGPT", "chatgpt", &text_clone, "@chatgpt", msg_id).await;
                });
                self.save_processed_id(msg_id);
            }
        }
        Ok(())
    }

    async fn handle_agent_trigger(&mut self, agent_name: &str, _page_key: &str, text: &str, trigger: &str, msg_id: usize) -> BridgeResult<()> {
        if self.busy_agents.contains(agent_name) {
            println!("Block: {} is busy.", agent_name);
            return Ok(());
        }

        let prompt = self.extract_prompt(text, trigger);
        self.busy_agents.insert(agent_name.to_string());
        self.save_processed_id(msg_id);

        // 1. Update status to Busy
        let _ = self.update_state(serde_json::json!({
            "agents": {
                agent_name: {
                    "status": "busy",
                    "current_task": prompt
                }
            }
        })).await;

        let name = agent_name.to_string();
        let name_lower = name.to_lowercase();
        
        let mut retries = 2; // Reduced retries to avoid detection/looping
        let mut success = false;
        while retries > 0 {
            let res = if name_lower == "chatgpt" {
                self.process_chatgpt_task(prompt.clone()).await
            } else if name_lower == "claude" {
                self.process_claude_task(prompt.clone()).await
            } else {
                Err("Unknown agent".into())
            };

            match res {
                Ok(_) => {
                    success = true;
                    break;
                },
                Err(e) => {
                    eprintln!("Error processing {} task (retries left: {}): {}", name, retries - 1, e);
                    retries -= 1;
                    if retries == 0 { 
                        self.busy_agents.remove(&name);
                        // Update status to Not Connected if failed (Since browser is closed)
                        let _ = self.update_state(serde_json::json!({
                            "agents": {
                                agent_name: {
                                    "status": "not connected",
                                    "current_task": "None",
                                    "last_task": format!("FAILED: {}", prompt)
                                }
                            }
                        })).await;
                        return Err(e); 
                    }
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }

        if success {
            // 2. Update status to Not Connected instead of Idle (Since browser is closed)
            let _ = self.update_state(serde_json::json!({
                "agents": {
                    agent_name: {
                        "status": "not connected",
                        "current_task": "None",
                        "last_task": prompt
                    }
                }
            })).await;
        }

        self.busy_agents.remove(&name);
        Ok(())
    }

    // --- ChatGPT Specialized Logic ---
    async fn process_chatgpt_task(&mut self, prompt: String) -> BridgeResult<()> {
        let name = "ChatGPT";
        self.post_message(name, "Thinking... Roger.").await?;
        
        let config = self.agent_configs.get("chatgpt").ok_or("ChatGPT config not found")?;
        let target_url = config.url.clone();
        
        // 1. Check for existing browser
        let mut browser_opt = None;
        {
            let map = self.active_browsers.lock().unwrap();
            if let Some(b) = map.get(name) {
                browser_opt = Some(b.clone());
            }
        }

        let browser = if let Some(b) = browser_opt {
            println!("Reusing existing ChatGPT browser session (Memory).");
            b
        } else {
            // Try explicit reconnection to port 9222 (True Persistence)
            let dev_tools_url = "http://127.0.0.1:9222/json/version";
            
            // Simplified Strategy:
            // 1. Attempt to fetch WS URL from localhost:9222
            // 2. If valid, Browser::connect
            // 3. If invalid, Browser::launch with --remote-debugging-port=9222
            
            let mut final_browser = None;
            let mut final_handler = None;
            
            // Try Connect
            match reqwest::get(dev_tools_url).await {
                Ok(resp) => {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        if let Some(ws_url) = json.get("webSocketDebuggerUrl").and_then(|v| v.as_str()) {
                            println!("Found existing Chrome at 9222. Connecting...");
                            if let Ok((b, h)) = Browser::connect(ws_url).await {
                                final_browser = Some(b);
                                final_handler = Some(h);
                            }
                        }
                    }
                }
                Err(_) => { /* Port closed, proceed to launch */ }
            }

            if final_browser.is_none() {
                println!("Launching NEW ChatGPT browser session (Port 9222).");
                let agent_data_dir = format!("{}/chatgpt", USER_DATA_DIR);
                let browser_cfg = self.create_browser_config(agent_data_dir, Some(9222))?;
                
                let (b, h) = Browser::launch(browser_cfg).await?;
                final_browser = Some(b);
                final_handler = Some(h);
            }

            let browser = final_browser.unwrap();
            let mut handler = final_handler.unwrap();

            // Spawn handler to keep running
            tokio::spawn(async move {
                while let Some(h) = handler.next().await {
                   if let Err(e) = h { eprintln!("Browser handler error: {}", e); break; }
                }
                println!("Browser handler ended.");
            });

            let arc_browser = Arc::new(browser);
            // Store in map
            {
                let mut map = self.active_browsers.lock().unwrap();
                map.insert(name.to_string(), arc_browser.clone());
            }
            arc_browser
        };

        // 2. Perform Interaction (Reuse active browser)
        // We intentionally DO NOT listen for disconnect_rx here in the same way 
        // because we want it to persist. 
        // However, we should handle if the browser is effectively dead?
        // perform_chatgpt_interaction calls new_page usually. 
        // We might want to find an existing page?
        
        let res = self.perform_chatgpt_interaction(&browser, &target_url, &prompt).await;

        // 3. DO NOT CLOSE
        // let _ = browser.close().await; 
        
        self.busy_agents.remove(name);
        
        if let Err(e) = &res {
            // If interaction failed, maybe the browser is dead?
            // For safety, we could remove it from the map if it's a connection error.
            eprintln!("Interaction failed: {}", e);
            // Optional: remove from map if critical error
        }
        
        res
    }

    async fn perform_chatgpt_interaction(&self, browser: &Browser, url: &str, prompt: &str) -> BridgeResult<()> {
        // Try to find existing page matching URL or create new
        let pages = browser.pages().await?;
        let mut found_page = None;
        for p in &pages {
            if let Ok(Some(u)) = p.url().await {
                if u.contains("chatgpt.com") {
                    found_page = Some(p.clone());
                    break;
                }
            }
        }

        let page = if let Some(p) = found_page {
            println!("Attaching to existing ChatGPT tab.");
            p.activate().await?;
            p
        } else {
            println!("Opening new ChatGPT tab.");
            browser.new_page(url).await?
        };

        let input_sel = "div#prompt-textarea";
        let send_btn_sel = "button[data-testid=\"send-button\"], button[aria-label=\"Send prompt\"]";
        
        // 1. Handle Overlays (Cookie Banners, etc.) & CHECK FOR INTERVENTION
        // Check for "Log in" or "Restore pages"
        let mut needs_help = false;
        loop {
            // Restore pages check
            let restore_btn = page.find_element("button:contains('Restore')").await; // Pseudo-selector logic needed or JS
            // Chromiumoxide doesn't support :contains unfortunately. We need JS check.
            
            let intervention_js = r#"() => {
                const text = document.body.innerText;
                const loginParams = text.includes("Log in") && text.includes("Sign up");
                const restoreParams = text.includes("Restore pages") && text.includes("Chrome didn't shut down correctly");
                return loginParams || restoreParams;
            }"#;

            match page.evaluate(intervention_js).await {
                Ok(val) => {
                     if let Some(true) = val.value().and_then(|v| v.as_bool()) {
                         if !needs_help {
                             println!("🔒 INTERVENTION REQUIRED: Log in / Restore detected. Calling Commander...");
                             self.update_agent_status("ChatGPT", "ASSISTANCE REQUIRED", "Waiting for Commander...").await?;
                             needs_help = true;
                         }
                         sleep(Duration::from_secs(2)).await;
                         continue;
                     }
                },
                Err(_) => {} 
            }
            
            if needs_help {
                println!("✅ Intervention cleared. Resuming...");
                self.update_agent_status("ChatGPT", "busy", prompt).await?;
                needs_help = false;
            }
            break;
        }

        let _ = self.handle_chatgpt_overlays(&page).await;
        sleep(Duration::from_millis(500)).await;

        self.update_agent_status("ChatGPT", "busy", prompt).await?;

        // 2. Focus and Type
        let input = self.wait_for_selector(&page, input_sel).await?;
        input.scroll_into_view().await?;
        
        if let Ok(Some(coords)) = self.get_element_center(&page, input_sel).await {
            let _ = page.move_mouse(Point::new(coords.0, coords.1)).await;
            sleep(Duration::from_millis(200)).await;
        }

        input.click().await?;
        input.focus().await?;
        
        // JS-based input injection to support Korean/CJK without key-map panic
        let content_json = serde_json::to_string(prompt)?;
        let input_js = format!(r#"
            const el = document.querySelector("{}");
            if (el) {{
                el.focus();
                // Set text precisely
                document.execCommand('insertText', false, {});
                // Fallback if execCommand fails
                if (el.innerText.trim() === "") {{
                    el.innerText = {};
                }}
                el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                el.dispatchEvent(new Event('change', {{ bubbles: true }}));
            }}
        "#, input_sel, content_json, content_json);
        
        let _ = page.evaluate(input_js).await;
        sleep(Duration::from_millis(500)).await;

        // 3. Explicitly click the Send Button (Up Arrow)
        if let Ok(btn) = self.wait_for_selector(&page, send_btn_sel).await {
            println!("Clicking ChatGPT send button...");
            btn.scroll_into_view().await?;
            btn.click().await?;
        } else {
            println!("Send button not found, falling back to Enter...");
            input.press_key("Enter").await?;
        }

        self.wait_for_settle(&page).await?;
        
        let _ = page.evaluate("document.dispatchEvent(new KeyboardEvent('keydown', {'key': 'Escape'}))").await;
        
        // Debug Screenshot
        let _ = self.take_screenshot(&page, "debug_chatgpt.png").await;
        
        // Extraction specialized for modern ChatGPT
        let answer = self.scrape_chatgpt_response(&page).await?.unwrap_or_else(|| "Extraction failed.".to_string());
        self.post_message("ChatGPT", &answer).await?;
        
        self.update_agent_status("ChatGPT", "idle", "").await?;
        Ok(())
    }

    async fn handle_chatgpt_overlays(&self, page: &Page) -> BridgeResult<()> {
        // Handle Cookie Banner: "Accept all"
        let cookie_js = r#"() => {
            const btns = [...document.querySelectorAll('button')];
            const acceptBtn = btns.find(b => b.innerText.includes('Accept all'));
            if (acceptBtn) {
                acceptBtn.click();
                return "Cookie banner clicked";
            }
            return "No cookie banner found";
        }"#;
        
        if let Ok(res) = page.evaluate(cookie_js).await {
            if let Some(msg) = res.value().and_then(|v| v.as_str()) {
                if msg.contains("clicked") { println!("ChatGPT Overlay: {}", msg); }
            }
        }
        
        // Universal Escape to clear other modals
        let _ = page.evaluate("document.dispatchEvent(new KeyboardEvent('keydown', {'key': 'Escape'}))").await;
        Ok(())
    }

    async fn scrape_chatgpt_response(&self, page: &Page) -> BridgeResult<Option<String>> {
        // More robust selectors for GPT-4o
        let sel = ".markdown, .prose, div[data-message-author-role=\"assistant\"]";
        self.scrape_last_message(page, sel).await
    }

    // --- Claude Specialized Logic ---
    async fn process_claude_task(&mut self, prompt: String) -> BridgeResult<()> {
        let name = "Claude";
        self.post_message(name, "Thinking... Roger.").await?;
        
        let config = self.agent_configs.get("claude").ok_or("Claude config not found")?;
        let target_url = config.url.clone();
        
        // 1. Check for existing browser
        let mut browser_opt = None;
        {
            let map = self.active_browsers.lock().unwrap();
            if let Some(b) = map.get(name) {
                browser_opt = Some(b.clone());
            }
        }

        let browser = if let Some(b) = browser_opt {
            println!("Reusing existing Claude browser session (Memory).");
            b
        } else {
            // Try explicit reconnection to port 9223 (Claude's dedicated port)
            let dev_tools_url = "http://127.0.0.1:9223/json/version";
            
            let mut final_browser = None;
            let mut final_handler = None;
            
            // Try Connect to existing browser on port 9223
            match reqwest::get(dev_tools_url).await {
                Ok(resp) => {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        if let Some(ws_url) = json.get("webSocketDebuggerUrl").and_then(|v| v.as_str()) {
                            println!("Found existing Chrome at 9223 (Claude). Connecting...");
                            if let Ok((b, h)) = Browser::connect(ws_url).await {
                                final_browser = Some(b);
                                final_handler = Some(h);
                            }
                        }
                    }
                }
                Err(_) => { /* Port closed, proceed to launch */ }
            }

            if final_browser.is_none() {
                println!("Launching NEW Claude browser session (Port 9223).");
                let agent_data_dir = format!("{}/claude", USER_DATA_DIR);
                let browser_cfg = self.create_browser_config(agent_data_dir, Some(9223))?;
                
                let (b, h) = Browser::launch(browser_cfg).await?;
                final_browser = Some(b);
                final_handler = Some(h);
            }

            let browser = final_browser.unwrap();
            let mut handler = final_handler.unwrap();

            // Spawn handler to keep running
            tokio::spawn(async move {
                while let Some(h) = handler.next().await {
                   if let Err(e) = h { eprintln!("Claude browser handler error: {}", e); break; }
                }
                println!("Claude browser handler ended.");
            });

            let arc_browser = Arc::new(browser);
            {
                let mut map = self.active_browsers.lock().unwrap();
                map.insert(name.to_string(), arc_browser.clone());
            }
            arc_browser
        };

        // 2. Perform Interaction without disconnect race
        let res = self.perform_claude_interaction(&browser, &target_url, &prompt).await;

        self.busy_agents.remove(name);
        res
    }

    async fn perform_claude_interaction(&self, browser: &Browser, url: &str, prompt: &str) -> BridgeResult<()> {
        // Try to find existing Claude page first
        let pages = browser.pages().await?;
        let mut found_page = None;
        for p in &pages {
            if let Ok(Some(u)) = p.url().await {
                if u.contains("claude.ai") {
                    found_page = Some(p.clone());
                    break;
                }
            }
        }

        let page = if let Some(p) = found_page {
            println!("Attaching to existing Claude tab.");
            p.activate().await?;
            sleep(Duration::from_secs(2)).await; // Stabilization after tab switch
            p
        } else {
            println!("Opening new Claude tab.");
            let new_page = browser.new_page(url).await?;
            println!("Claude tab opened, waiting for page load...");
            sleep(Duration::from_secs(3)).await; // CRITICAL: Wait for page to fully load
            new_page
        };
        
        let input_sel = "div[contenteditable=\"true\"]";
        
        self.update_agent_status("Claude", "busy", prompt).await?;

        // --- Intervention Protocol Start ---
        let mut needs_help = false;
        let mut check_count = 0;
        
        loop {
            // 1. SUCCESS CONDITION: Input found
            if let Ok(_) = page.find_element(input_sel).await {
                if needs_help {
                     println!("✅ Intervention cleared. Input found. Resuming Claude...");
                     self.update_agent_status("Claude", "busy", prompt).await?;
                }
                break;
            }

            // 2. DETECT INTERVENTION
            let intervention_js = r#"() => {
                const text = document.body.innerText;
                const loginParams = text.includes("Log in") || text.includes("Sign up") || text.includes("Welcome") || text.includes("Continue with") || text.includes("email") || text.includes("Google");
                const restoreParams = text.includes("Restore pages") && text.includes("Chrome didn't shut down correctly");
                return loginParams || restoreParams;
            }"#;

            let detected = match page.evaluate(intervention_js).await {
                Ok(val) => val.value().and_then(|v| v.as_bool()).unwrap_or(false),
                Err(_) => false
            };

            if detected || needs_help {
                 if !needs_help {
                     println!("🔒 INTERVENTION REQUIRED (Claude): Log in / Restore detected. Calling Commander...");
                     self.update_agent_status("Claude", "ASSISTANCE REQUIRED", "Waiting for Commander (Login)...").await?;
                     needs_help = true;
                 }
                 // If we are in help mode, we just wait.
                 sleep(Duration::from_secs(2)).await;
                 continue; 
            }
            
            // 3. AMBIGUOUS STATE (No input, No login detected yet)
            check_count += 1;
            if check_count > 15 { // ~30 seconds of limbo
                 println!("⚠️  Wait timeout approaching, checking specific selectors...");
            }
            sleep(Duration::from_secs(2)).await;
        }
        // --- Intervention Protocol End ---

        let input = self.wait_for_selector(&page, input_sel).await?;
        input.scroll_into_view().await?;
        sleep(Duration::from_millis(1000)).await;
        input.click().await?;
        sleep(Duration::from_millis(1000)).await; // Focus wait

        // NATIVE CTRL+V STRATEGY
        // 1. Write to Clipboard using JS
        let content_json = serde_json::to_string(prompt)?;
        let clipboard_js = format!(r#"
            navigator.clipboard.writeText({}).then(() => {{
                console.log("Copied to clipboard");
            }}).catch(err => {{
                console.error("Clipboard failed:", err);
            }});
        "#, content_json);
        let _ = page.evaluate(clipboard_js).await;
        // 2. Simulate Physical Ctrl+V Keys using low-level API (CDP)
        // Control Down (No text generated)
        let cmd_down = DispatchKeyEventParams::builder()
            .r#type(DispatchKeyEventType::KeyDown)
            .modifiers(2) // 2 = Control
            .key("Control")
            .code("ControlLeft")
            .windows_virtual_key_code(17)
            .build()
            .unwrap();
        page.execute(cmd_down).await?;

        // V Down (with Control modifier still active)
        let cmd_v = DispatchKeyEventParams::builder()
            .r#type(DispatchKeyEventType::KeyDown)
            .modifiers(2) 
            // Control+V produces no text, so do not set text/unmodified_text
            .key("v")
            .code("KeyV")
            .windows_virtual_key_code(86)
            .build()
            .unwrap();
        page.execute(cmd_v).await?;

        // V Up
        let cmd_v_up = DispatchKeyEventParams::builder()
            .r#type(DispatchKeyEventType::KeyUp)
            .modifiers(2)
            .key("v")
            .code("KeyV")
            .windows_virtual_key_code(86)
            .build()
            .unwrap();
        page.execute(cmd_v_up).await?;

        // Control Up
        let cmd_up = DispatchKeyEventParams::builder()
            .r#type(DispatchKeyEventType::KeyUp)
            .key("Control")
            .code("ControlLeft")
            .windows_virtual_key_code(17)
            .build()
            .unwrap();
        page.execute(cmd_up).await?;
        
        sleep(Duration::from_secs(2)).await; // Wait for paste

        // Try Clicking Send Button explicitly
        let send_btn_sel = "button[aria-label=\"Send Message\"], button[data-testid=\"send-button\"]";
        if let Ok(btn) = self.wait_for_selector(&page, send_btn_sel).await {
             println!("Clicking Claude send button...");
             btn.click().await?;
        } else {
             println!("Claude send button not found, trying Enter...");
             // Direct Enter key simulation via CDP
             let enter_cmd = DispatchKeyEventParams::builder()
                .r#type(DispatchKeyEventType::KeyDown)
                .text("\r")
                .unmodified_text("\r")
                .key("Enter")
                .code("Enter")
                .windows_virtual_key_code(13)
                .build()
                .unwrap();
             let _ = page.execute(enter_cmd).await;
        }

        self.wait_for_settle(&page).await?;
        
        let sel = ".font-claude-message, [data-testid=\"assistant-message\"], .prose";
        let answer = self.scrape_last_message(&page, sel).await?.unwrap_or_else(|| "Extraction failed.".to_string());
        self.post_message("Claude", &answer).await?;
        
        self.update_agent_status("Claude", "idle", "").await?;
        Ok(())
    }

    // --- Helpers ---
    fn cleanup_browser_process(&self, data_dir: &str) -> BridgeResult<()> {
        let lock_path = format!("{}/SingletonLock", data_dir);
        if std::path::Path::new(&lock_path).exists() {
            println!("Cleaning up stale browser lock: {}", lock_path);
            let _ = fs::remove_file(lock_path);
        }
        Ok(())
    }

    fn create_browser_config(&self, data_dir: String, debug_port: Option<u16>) -> BridgeResult<BrowserConfig> {
        let mut builder = BrowserConfig::builder()
            .user_data_dir(PathBuf::from(data_dir))
            .headless_mode(HeadlessMode::False)
            .disable_default_args()
            .arg("--no-first-run")
            .arg("--password-store=basic")
            .arg("--use-mock-keychain")
            .arg("--no-default-browser-check")
            .arg("--disable-session-crashed-bubble")
            .arg("--disable-blink-features=AutomationControlled")
            .arg("--no-sandbox")
            .arg("--disable-infobars")
            .arg("--disable-component-update")
            .arg("--restore-last-session=false")
            .arg("--exit-type=Normal")
            .window_size(1280, 800);

        let port_str;
        if let Some(port) = debug_port {
            port_str = format!("--remote-debugging-port={}", port);
            builder = builder.arg(&port_str);
        }

        builder.build().map_err(|e| e.into())
    }

    async fn update_agent_status(&self, name: &str, status: &str, task: &str) -> BridgeResult<()> {
        let mut updates = HashMap::new();
        let mut agent_map = HashMap::new();
        agent_map.insert("status", status.to_string());
        if !task.is_empty() {
            let status_text = if task.chars().count() > 50 { 
                format!("{}...", task.chars().take(50).collect::<String>()) 
            } else { 
                task.to_string() 
            };
            agent_map.insert("current_task", status_text);
        } else {
            agent_map.insert("current_task", "None".to_string());
        }
        let mut inner = HashMap::new();
        inner.insert(name.to_string(), agent_map);
        updates.insert("agents", inner);
        self.update_state(serde_json::to_value(updates)?).await
    }

    async fn get_element_center(&self, page: &Page, selector: &str) -> BridgeResult<Option<(f64, f64)>> {
        let js = format!(r#"() => {{
            const el = document.querySelector("{}");
            if (!el) return null;
            const rect = el.getBoundingClientRect();
            return [window.scrollX + rect.left + rect.width / 2, window.scrollY + rect.top + rect.height / 2];
        }}"#, selector.replace("\"", "\\\""));
        
        let val = page.evaluate(js).await?.value().and_then(|v| v.as_array().and_then(|a| {
            if a.len() == 2 {
                Some((a[0].as_f64().unwrap_or(0.0), a[1].as_f64().unwrap_or(0.0)))
            } else { None }
        }));
        Ok(val)
    }


    fn extract_prompt(&self, text: &str, trigger: &str) -> String {
        if let Some(pos) = text.to_lowercase().find(&trigger.to_lowercase()) {
            text[pos + trigger.len()..].trim().to_string()
        } else {
            text.trim().to_string()
        }
    }

    async fn run_audit_command() -> BridgeResult<()> {
        println!("Executing sentry audit...");
        let output = std::process::Command::new("./target/release/sentry")
            .arg("--once")
            .output()?;
        
        if output.status.success() {
            let report = String::from_utf8_lossy(&output.stdout);
            let client = Client::new();
            let _ = client.post(format!("{}/messages", API_BASE))
                .json(&serde_json::json!({
                    "sender": "Antigravity",
                    "message": format!("**PROJECT SENTRY REPORT**\n\n{}", report)
                }))
                .send()
                .await;
        }
        Ok(())
    }

    async fn wait_for_settle(&self, page: &Page) -> BridgeResult<()> {
        println!("Waiting for generation to settle...");
        
        // Inject MutationObserver-based wait logic
        page.evaluate(r#"
            window.__waitForAssistant = () => new Promise(resolve => {
                let lastChange = Date.now();
                const obs = new MutationObserver(() => { lastChange = Date.now(); });
                obs.observe(document.body, { childList: true, subtree: true, characterData: true });
                
                const check = setInterval(() => {
                    const silence = Date.now() - lastChange;
                    if (silence > 2000) { // Increased to 2.0s for stability
                        clearInterval(check);
                        obs.disconnect();
                        resolve(true);
                    }
                }, 300);
                
                setTimeout(() => {
                    clearInterval(check);
                    obs.disconnect();
                    resolve(true);
                }, 60000);
            });
        "#).await?;
        
        page.evaluate("window.__waitForAssistant()").await?;
        println!("Generation settled.");
        Ok(())
    }

    async fn scrape_last_message(&self, page: &Page, selector: &str) -> BridgeResult<Option<String>> {
        let js_cmd = format!(r#"() => {{
            const selectors = "{}".split(",").map(s => s.trim());
            let debugInfo = [];
            for (const sel of selectors) {{
                const elements = [...document.querySelectorAll(sel)];
                debugInfo.push(`Selector '${{sel}}' found ${{elements.length}} elements`);

                // Filter specifically for assistant role if possible
                const assistants = elements.filter(el => 
                    el.getAttribute('data-message-author-role') === 'assistant' || 
                    el.closest('[data-message-author-role="assistant"]') ||
                    (el.classList.contains('prose') && !el.closest('[data-message-author-role="user"]'))
                );
                
                debugInfo.push(`  -> ${{assistants.length}} passed filter`);

                if (assistants.length > 0) {{
                    const last = assistants[assistants.length - 1];
                    const text = last.innerText.trim();
                    if (text.length > 3) return text;
                    debugInfo.push(`  -> Last element text too short: ${{text.length}}`);
                }}
            }}
            return "Debug: " + debugInfo.join(" | ");
        }}"#, selector.replace("\"", "\\\""));

        let result = page.evaluate(js_cmd).await?;
        let text_opt = result.value().and_then(|v| v.as_str().map(|s| s.to_string()));
        
        if let Some(text) = text_opt {
            if text.starts_with("Debug:") {
                println!("⚠️ Extraction Failed Details: {}", text);
                return Ok(None);
            }
            return Ok(Some(text));
        }
        Ok(None)
    }

    async fn post_message(&self, sender: &str, text: &str) -> BridgeResult<()> {
        let _ = self.client.post(format!("{}/messages", API_BASE))
            .json(&serde_json::json!({
                "sender": sender,
                "message": text
            }))
            .send()
            .await;
        Ok(())
    }

    async fn update_state(&self, updates: Value) -> BridgeResult<()> {
        let _ = self.client.post(format!("{}/state", API_BASE))
            .json(&updates)
            .send()
            .await;
        Ok(())
    }

    async fn wait_for_selector(&self, page: &Page, selector: &str) -> BridgeResult<chromiumoxide::Element> {
        for _ in 0..10 {
            if let Ok(el) = page.find_element(selector).await {
                return Ok(el);
            }
            sleep(Duration::from_millis(1000)).await;
        }
        Err(format!("Timeout waiting for selector: {}", selector).into())
    }

    async fn take_screenshot(&self, page: &Page, filename: &str) -> BridgeResult<()> {
        let mut path = PathBuf::from(USER_DATA_DIR);
        path.push(filename);
        page.save_screenshot(chromiumoxide::page::ScreenshotParams::builder().full_page(true).build(), &path).await?;
        println!("Screenshot saved to {:?}", path);
        Ok(())
    }

    async fn inject_briefings(&self, _msg_id: usize) -> BridgeResult<()> {
        let briefing_path = "briefings/tactical_brief.md";
        
        if let Ok(content) = fs::read_to_string(briefing_path) {
            println!("Injecting briefing for ChatGPT (On-Demand)...");
            
            let name = "chatgpt";
            let config = self.agent_configs.get(name).ok_or("Agent config not found")?;
            let input_selector = "div#prompt-textarea";
            let target_url = config.url.clone();
            
            // Launch Temporary Browser
            let agent_data_dir = format!("{}/{}", USER_DATA_DIR, name);
            let browser_cfg = BrowserConfig::builder()
                .user_data_dir(PathBuf::from(agent_data_dir))
                .headless_mode(HeadlessMode::False)
                .disable_default_args()
                .arg("--no-first-run")
                .arg("--password-store=basic")
                .arg("--use-mock-keychain")
                .arg("--no-default-browser-check")
                .arg("--disable-session-crashed-bubble")
                .arg("--disable-blink-features=AutomationControlled")
                .arg("--no-sandbox")
                .arg("--disable-infobars")
                .arg("--disable-component-update")
                .window_size(1280, 800)
                .build()?;

            let (mut browser, mut handler) = Browser::launch(browser_cfg).await?;
            tokio::spawn(async move {
                while let Some(h) = handler.next().await {
                    if let Err(_) = h { break; }
                }
            });

            let page = browser.new_page(target_url).await?;
            let _ = page.evaluate("document.dispatchEvent(new KeyboardEvent('keydown', {'key': 'Escape'}))").await;

            if let Ok(input) = self.wait_for_selector(&page, input_selector).await {
                let _ = input.click().await;
                let _ = input.focus().await;
                
                let content_json = serde_json::to_string(&content)?;
                let _ = page.evaluate(format!(r#"
                    const el = document.querySelector("{}");
                    el.innerHTML = "<p>" + {} + "</p>";
                    el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                "#, input_selector, content_json)).await;
                
                let _ = input.press_key("Enter").await;
                self.post_message("Antigravity", "Briefing injection completed for ChatGPT.").await?;
            }

            let _ = browser.close().await;
        } else {
            println!("Briefing file not found: {}", briefing_path);
            self.post_message("Antigravity", "Briefing file not found.").await?;
        }
        
        Ok(())
    }

    fn clone_for_task(&self) -> Self {
        Self {
            last_message_id: self.last_message_id,
            processed_ids: self.processed_ids.clone(),
            busy_agents: self.busy_agents.clone(),
            agent_configs: self.agent_configs.clone(),
            client: Client::new(),
            active_browsers: self.active_browsers.clone(),
        }
    }
}
