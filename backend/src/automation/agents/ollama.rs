use crate::automation::agents::protocol::{AgentContext, AgentResult};
use serde_json::json;

pub async fn execute_ollama_task(ctx: &AgentContext, prompt: &str) -> AgentResult<()> {
    println!("[Ollama] Starting task with model qwen3:8b: {}", prompt.chars().take(50).collect::<String>());
    ctx.update_status("busy", prompt).await?;

    let payload = json!({
        "model": "qwen3:8b",
        "prompt": prompt,
        "stream": false
    });

    let resp = ctx.http_client.post("http://localhost:11434/api/generate")
        .json(&payload)
        .send()
        .await?;

    if !resp.status().is_success() {
        let err_text = resp.text().await?;
        ctx.post_message(&format!("Ollama Error: {}", err_text)).await?;
        ctx.update_status("idle", "").await?;
        return Err(format!("Ollama API failed: {}", err_text).into());
    }

    let res_json: serde_json::Value = resp.json().await?;
    let answer = res_json["response"].as_str().unwrap_or("Failed to get response from Ollama.");

    println!("[Ollama] Received response from Qwen3");
    
    // Append "Over." to match ACP protocol if not present
    let final_answer = if answer.trim().to_lowercase().ends_with("over.") {
        answer.to_string()
    } else {
        format!("{}\n\nOver.", answer)
    };

    ctx.post_message(&final_answer).await?;
    ctx.update_status("idle", "").await?;

    Ok(())
}
