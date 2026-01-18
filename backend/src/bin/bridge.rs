use backend::automation::BridgeManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (mut bridge, state_actor) = BridgeManager::new().await;
    
    // Run state actor in background
    tokio::spawn(state_actor.run());
    
    // Run bridge manager (orchestrator)
    bridge.run().await;
    
    Ok(())
}
