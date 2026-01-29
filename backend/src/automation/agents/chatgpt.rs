/* 
 * HBI (Human Behavior Imitation) Protocol Implementation
 * 
 * 1. Base Navigation: Always start at the root URL to avoid 'bot-suspicious' URL patterns.
 * 2. Visual Interaction: Interacts only with visible UI elements to maintain a normal session state.
 * 3. Human Typing: Uses DOM 'insertText' instead of setting .value to trigger proper React/Vue emitters.
 * 4. Settle Detection: Waits for visual stability before extracting data.
 */

use tokio::time::{sleep, Duration};
use crate::automation::agents::protocol::{AgentContext, AgentResult, get_page};
use chromiumoxide::Page;
use rand::Rng;

pub async fn execute_chatgpt_task(ctx: &AgentContext, prompt: &str) -> AgentResult<()> {
    println!("[ChatGPT] Starting task: {}", prompt.chars().take(50).collect::<String>());
    let url = "https://chatgpt.com/";
    let page = get_page(&ctx.browser_tx, url).await?;

    // HBI Step: Suppression Scripting - Hide automation flag early
    println!("[ChatGPT] Injecting HBI scripts...");
    let _ = page.evaluate("Object.defineProperty(navigator, 'webdriver', {get: () => undefined})").await?;

    let input_sel = "div#prompt-textarea";
    let send_btn_sel = "button[data-testid=\"send-button\"], button[aria-label=\"Send prompt\"]";

    ctx.update_status("busy", prompt).await?;

    // HBI Step: Organic Focus with Randomized Jitter
    let (delay1, delay2) = {
        let mut rng = rand::rng();
        (rng.random_range(300..900), rng.random_range(500..1200))
    };
    sleep(Duration::from_millis(delay1)).await;
    
    println!("[ChatGPT] Focusing input...");
    let input = page.find_element(input_sel).await?;
    input.click().await?;
    sleep(Duration::from_millis(delay2)).await;
    
    println!("[ChatGPT] Typing prompt...");
    
    let content_json = serde_json::to_string(prompt)?;
    let input_js = format!(r#"
        const el = document.querySelector("{}");
        if (el) {{
            el.focus();
            // Human Behavioral Pattern: Imitating standard OS clipboard/typing insertion
            document.execCommand('insertText', false, {});
            el.dispatchEvent(new Event('input', {{ bubbles: true }}));
        }}
    "#, input_sel, content_json);
    
    let delay3 = {
        let mut rng = rand::rng();
        rng.random_range(1000..2500)
    };
    let _ = page.evaluate(input_js).await?;
    sleep(Duration::from_millis(delay3)).await; // Natural human hesitation

    println!("[ChatGPT] Sending prompt...");
    // Click Send with safety fallback
    if let Ok(btn) = page.find_element(send_btn_sel).await {
        btn.click().await?;
    } else {
        let _ = page.evaluate("document.activeElement.dispatchEvent(new KeyboardEvent('keydown', {key: 'Enter', bubbles: true}))").await;
    }

    println!("[ChatGPT] Waiting for assistant to settle...");
    wait_for_settle(&page).await?;
    
    // Capture debug screenshot
    let screenshot_path = format!("./isolated_data/chatgpt/last_execution.png");
    let _ = std::fs::create_dir_all("./isolated_data/chatgpt");
    let _ = page.save_screenshot(chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotParams::default(), &screenshot_path).await;
    println!("[ChatGPT] Screenshot saved to {}", screenshot_path);
    
    // Scrape result
    println!("[ChatGPT] Scraping response...");
    let answer = scrape_response(&page).await?.unwrap_or_else(|| "Extraction failed.".to_string());
    println!("[ChatGPT] Extraction result: {}", answer.chars().take(50).collect::<String>());
    ctx.post_message(&answer).await?;
    
    ctx.update_status("idle", "").await?;
    Ok(())
}

async fn wait_for_settle(page: &Page) -> AgentResult<()> {
    page.evaluate(r#"
        window.__waitForAssistant = () => new Promise(resolve => {
            let lastChange = Date.now();
            const maxWait = 60000; // 60s timeout (Increased as per Commander's request)
            const start = Date.now();
            const obs = new MutationObserver(() => { lastChange = Date.now(); });
            obs.observe(document.body, { childList: true, subtree: true, characterData: true });
            const check = setInterval(() => {
                const now = Date.now();
                if (now - lastChange > 3000 || now - start > maxWait) {
                    clearInterval(check);
                    obs.disconnect();
                    resolve(true);
                }
            }, 300);
        });
    "#).await?;
    page.evaluate("window.__waitForAssistant()").await?;
    Ok(())
}

async fn scrape_response(page: &Page) -> AgentResult<Option<String>> {
    // Obsidian-inspired robust scraping: Target the structured article/prose of the LAST assistant message
    let js = r#"() => {
        const articles = [...document.querySelectorAll('article')];
        const assistantArticles = articles.filter(art => art.querySelector('[data-message-author-role="assistant"]'));
        
        if (assistantArticles.length > 0) {
            const lastAssistant = assistantArticles[assistantArticles.length - 1];
            const prose = lastAssistant.querySelector('.prose');
            if (prose) {
                // Return clean text, preserving some structure if possible, or just innerText
                return prose.innerText.trim();
            }
            return lastAssistant.innerText.trim();
        }
        
        // Fallback to simpler selector if article structure is missing
        const fallbacks = document.querySelectorAll('[data-message-author-role="assistant"]');
        if (fallbacks.length > 0) return fallbacks[fallbacks.length - 1].innerText.trim();
        
        return null;
    }"#;
    
    let result = page.evaluate(js).await?;
    Ok(result.value().and_then(|v| v.as_str().map(|s| s.to_string())))
}
