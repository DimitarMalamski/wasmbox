use std::sync::Arc;

use tokio::sync::Semaphore;

use wasmbox::sandbox::SandboxConfig;

#[derive(Clone)]
pub struct AppState {
    pub execution_semaphore: Arc<Semaphore>,
    pub sandbox_config: SandboxConfig,
}
