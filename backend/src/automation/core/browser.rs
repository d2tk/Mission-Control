use tokio::sync::{mpsc, oneshot, Mutex};
use chromiumoxide::{Browser, BrowserConfig, Page};
use std::sync::Arc;
use tokio::task::JoinHandle;
use std::time::Instant;
use futures::StreamExt;

#[derive(Debug)]
pub enum BrowserCommand {
    GetPage { url: String, persistent: bool, reply: oneshot::Sender<Result<Page, String>> },
    Ping { reply: oneshot::Sender<bool> },
    Close,
}

pub struct BrowserActor {
    browser: Arc<Mutex<Browser>>,
    handler: Option<JoinHandle<()>>,
    _created_at: Instant,
    task_count: usize,
    rx: mpsc::Receiver<BrowserCommand>,
}

impl BrowserActor {
    pub async fn new(config: BrowserConfig, rx: mpsc::Receiver<BrowserCommand>) -> Result<Self, String> {
        let (browser, mut handler) = Browser::launch(config).await.map_err(|e| e.to_string())?;
        
        let handler_task = tokio::spawn(async move {
            while let Some(_) = handler.next().await {}
        });

        Ok(Self {
            browser: Arc::new(Mutex::new(browser)),
            handler: Some(handler_task),
            _created_at: Instant::now(),
            task_count: 0,
            rx,
        })
    }

    pub async fn run(mut self) {
        while let Some(cmd) = self.rx.recv().await {
            match cmd {
                BrowserCommand::GetPage { url, persistent: _, reply } => {
                    self.task_count += 1;
                    let browser = self.browser.lock().await;
                    match browser.new_page(url).await {
                        Ok(page) => {
                            let _ = reply.send(Ok(page));
                        }
                        Err(e) => {
                            let _ = reply.send(Err(e.to_string()));
                        }
                    }
                }
                BrowserCommand::Ping { reply } => {
                    let browser = self.browser.lock().await;
                    match browser.version().await {
                        Ok(_) => { let _ = reply.send(true); }
                        Err(_) => { let _ = reply.send(false); }
                    }
                }
                BrowserCommand::Close => {
                    let mut browser = self.browser.lock().await;
                    let _ = browser.close().await;
                    if let Some(h) = self.handler.take() {
                        h.abort();
                    }
                    break;
                }
            }
        }
    }
}
