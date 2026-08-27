use std::sync::Arc;

use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
};

use tokio::sync::Semaphore;

use super::{
    config::load_sandbox_config,
    handlers::{execute, health},
    state::AppState,
};

pub(super) const MAX_REQUEST_BYTES: usize = 1024 * 1024; // 1 MB
const MAX_CONCURRENT_EXECUTIONS: usize = 4;

pub(super) fn create_app() -> Result<Router, String> {
    let state = AppState {
        execution_semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_EXECUTIONS)),
        sandbox_config: load_sandbox_config()?,
    };

    Ok(Router::new()
        .route("/health", get(health))
        .route("/execute", post(execute))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BYTES))
        .with_state(state))
}
