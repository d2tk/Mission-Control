use backend::bridge::AIBridge;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut bridge = AIBridge::new();
    bridge.start().await?;
    Ok(())
}
