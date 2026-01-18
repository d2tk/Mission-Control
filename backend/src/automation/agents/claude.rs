/* 
 * HBI (Human Behavior Imitation) Protocol Implementation
 * 
 * 1. Base Navigation: Always start at the root URL to avoid 'bot-suspicious' URL patterns.
 * 2. Visual Interaction: Interacts only with visible UI elements to maintain a normal session state.
 * 3. Human Typing: Uses Ctrl+V simulation via CDP for Claude to handle large context safely.
 * 4. Cleanup: Clears clipboard immediately after use to maintain human-like hygiene and security.
 */

use tokio::time::{sleep, Duration};
use crate::automation::agents::protocol::{AgentContext, AgentResult, get_page};
use chromiumoxide::Page;

pub async fn execute_claude_task(ctx: &AgentContext, prompt: &str) -> AgentResult<()> {
    let url = "https://claude.ai/";
    let page = get_page(&ctx.browser_tx, url).await?;

    let input_sel = "div[contenteditable=\"true\"], div.ProseMirror";
    let send_btn_sel = "button[aria-label=\"Send Message\"], button[data-testid=\"send-button\"]";

    ctx.update_status("busy", prompt).await?;

    // HBI Step: Human-like Focus
    let input = page.find_element(input_sel).await?;
    input.click().await?;
    sleep(Duration::from_millis(500)).await;

    // Simulate Paste via CDP (Human behavior for large text)
    let _ = page.evaluate(format!("navigator.clipboard.writeText({})", serde_json::to_string(prompt)?)).await?;
    
    // Command-V or Ctrl-V based on platform simulation
    let _ = page.evaluate("document.activeElement.dispatchEvent(new KeyboardEvent('keydown', {key: 'v', ctrlKey: true, bubbles: true}))").await?;
    
    // Clear clipboard (Hygiene)
    let _ = page.evaluate("navigator.clipboard.writeText('')").await?;
    sleep(Duration::from_millis(1000)).await;

    // Click Send
    if let Ok(btn) = page.find_element(send_btn_sel).await {
        btn.click().await?;
    } else {
        let _ = page.evaluate("document.activeElement.dispatchEvent(new KeyboardEvent('keydown', {key: 'Enter', bubbles: true}))").await;
    }

    wait_for_settle(&page).await?;
    
    // Scrape result
    let answer = scrape_response(&page).await?.unwrap_or_else(|| "Extraction failed.".to_string());
    ctx.post_message(&answer).await?;
    
    ctx.update_status("idle", "").await?;
    Ok(())
}

async fn wait_for_settle(page: &Page) -> AgentResult<()> {
    page.evaluate(r#"
        window.__waitForAssistant = () => new Promise(resolve => {
            let lastChange = Date.now();
            const obs = new MutationObserver(() => { lastChange = Date.now(); });
            obs.observe(document.body, { childList: true, subtree: true, characterData: true });
            const check = setInterval(() => {
                if (Date.now() - lastChange > 2000) {
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
    let sel = ".font-claude-message, [data-testid=\"assistant-message\"], .prose";
    let js = format!(r#"() => {{
        const elements = [...document.querySelectorAll("{}")];
        if (elements.length > 0) return elements[elements.length - 1].innerText.trim();
        return null;
    }}"#, sel);
    
    let result = page.evaluate(js).await?;
    Ok(result.value().and_then(|v| v.as_str().map(|s| s.to_string())))
}
