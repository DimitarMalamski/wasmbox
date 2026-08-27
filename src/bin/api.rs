use wasmbox::sandbox::execute_wat;

use axum::{
    extract::{DefaultBodyLimit, State},
    routing::{get, post},
    Json, Router,
};

use std::sync::Arc;
use tokio::sync::Semaphore;

use serde::{Deserialize, Serialize};

const MAX_REQUEST_BYTES: usize = 1024 * 1024; // 1 MB
const MAX_CONCURRENT_EXECUTIONS: usize = 4;

#[derive(Clone)]
struct AppState {
    execution_semaphore: Arc<Semaphore>,
}

#[tokio::main]
async fn main() {
    let state = AppState {
        execution_semaphore: Arc::new(
            Semaphore::new(MAX_CONCURRENT_EXECUTIONS)
        ),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/execute", post(execute))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BYTES))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    println!("WasmBox API running at http://127.0.0.1:3000");

    axum::serve(listener, app)
        .await
        .unwrap();
}

async fn health() -> &'static str {
    "WasmBox is running"
}

#[derive(Deserialize)]
struct ExecuteRequest {
    code: String,
}

#[derive(Serialize)]
struct ExecuteResponse {
    success: bool,
    message: String,
    output: Vec<String>,
    execution_time_ms: Option<f64>,
    fuel_used: Option<u64>,
    memory_used_bytes: Option<usize>,
}

async fn execute(
    State(state): State<AppState>,
    Json(request): Json<ExecuteRequest>,
) -> Json<ExecuteResponse> {
    let permit = match state
        .execution_semaphore
        .clone()
        .try_acquire_owned()
    {
        Ok(permit) => permit,

        Err(_) => {
            return Json(ExecuteResponse {
                success: false,
                message: "Server is busy. Too many concurrent executions."
                    .to_string(),
                output: Vec::new(),
                execution_time_ms: None,
                fuel_used: None,
                memory_used_bytes: None,
            });
        }
    };

    let code = request.code;

    let execution = tokio::task::spawn_blocking(move || {
        let _permit = permit;

        execute_wat(&code)
    })
    .await;

    match execution {
        Ok(Ok(result)) => Json(ExecuteResponse {
            success: result.success,
            message: result.message,
            output: result.output,
            execution_time_ms: Some(result.execution_time_ms),
            fuel_used: Some(result.fuel_used),
            memory_used_bytes: Some(result.memory_used_bytes),
        }),

        Ok(Err(message)) => Json(ExecuteResponse {
            success: false,
            message,
            output: Vec::new(),
            execution_time_ms: None,
            fuel_used: None,
            memory_used_bytes: None,
        }),

        Err(error) => Json(ExecuteResponse {
            success: false,
            message: format!("Sandbox task failed: {}", error),
            output: Vec::new(),
            execution_time_ms: None,
            fuel_used: None,
            memory_used_bytes: None,
        }),
    }
}