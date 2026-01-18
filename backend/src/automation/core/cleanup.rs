use tokio::task::JoinHandle;
use tokio::sync::mpsc;

pub struct TaskId(pub usize);

pub struct TaskGuard<T> {
    handle: JoinHandle<T>,
    cleanup_tx: Option<mpsc::Sender<TaskId>>,
    id: TaskId,
}

impl<T> TaskGuard<T> {
    pub fn new(handle: JoinHandle<T>, cleanup_tx: mpsc::Sender<TaskId>, id: TaskId) -> Self {
        Self {
            handle,
            cleanup_tx: Some(cleanup_tx),
            id,
        }
    }

    pub fn spawn<F>(cleanup_tx: mpsc::Sender<TaskId>, future: F) -> Self 
    where 
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static
    {
        let id = TaskId(0); // This would normally be unique
        let handle = tokio::spawn(future);
        Self::new(handle, cleanup_tx, id)
    }
}

impl<T> Drop for TaskGuard<T> {
    fn drop(&mut self) {
        // Guarantee task termination on guard drop
        self.handle.abort();
        if let Some(tx) = self.cleanup_tx.take() {
            let id = TaskId(self.id.0);
            // Try to notify cleanup actor
            let _ = tx.try_send(id);
        }
    }
}
