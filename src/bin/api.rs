use wasmbox::sandbox::{
    execute_wat_with_config,
    ExecutionError,
    SandboxConfig,
    SandboxError,
};

use axum::{
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
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
    sandbox_config: SandboxConfig,
}

#[tokio::main]
async fn main() {
    let app = create_app();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    println!("WasmBox API running at http://127.0.0.1:3000");

    axum::serve(listener, app)
        .await
        .unwrap();
}

fn create_app() -> Router {
    let state = AppState {
        execution_semaphore: Arc::new(
            Semaphore::new(MAX_CONCURRENT_EXECUTIONS)
        ),
        sandbox_config: SandboxConfig::default(),
    };

    Router::new()
        .route("/health", get(health))
        .route("/execute", post(execute))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BYTES))
        .with_state(state)
}

async fn health() -> &'static str {
    "WasmBox is running"
}

#[derive(Deserialize)]
struct ExecuteRequest {
    code: String,
}

#[derive(Serialize, Deserialize)]
struct ExecuteResponse {
    success: bool,
    message: String,
    output: Vec<String>,
    execution_time_ms: Option<f64>,
    fuel_used: Option<u64>,
    memory_used_bytes: Option<usize>,
}

fn round_to_two_decimals(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

async fn execute(
    State(state): State<AppState>,
    Json(request): Json<ExecuteRequest>,
) -> (StatusCode, Json<ExecuteResponse>) {
    let permit = match state
        .execution_semaphore
        .clone()
        .try_acquire_owned()
    {
        Ok(permit) => permit,

        Err(_) => {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(ExecuteResponse {
                    success: false,
                    message: "Server is busy. Too many concurrent executions."
                        .to_string(),
                    output: Vec::new(),
                    execution_time_ms: None,
                    fuel_used: None,
                    memory_used_bytes: None,
                }),
            );
        }
    };

    let code = request.code;

    let sandbox_config = state.sandbox_config.clone();
    let execution = tokio::task::spawn_blocking(move || {
        let _permit = permit;

        execute_wat_with_config(
            &code,
            sandbox_config,
        )
    })
    .await;

    match execution {
        Ok(Ok(result)) => {
            let status = match &result.error {
                None => StatusCode::OK,

                Some(ExecutionError::Timeout) => {
                    StatusCode::REQUEST_TIMEOUT
                }

                Some(
                    ExecutionError::FuelExhausted
                    | ExecutionError::InvalidMemoryAccess
                    | ExecutionError::InvalidPointer
                    | ExecutionError::InvalidTextLength
                    | ExecutionError::InvalidMemoryRange
                    | ExecutionError::InvalidUtf8
                    | ExecutionError::OutputLimitExceeded
                    | ExecutionError::Other(_),
                ) => {
                    StatusCode::UNPROCESSABLE_ENTITY
                }
            };

            (
                status,
                Json(ExecuteResponse {
                    success: result.success,
                    message: result.message,
                    output: result.output,
                    execution_time_ms: Some(
                        round_to_two_decimals(result.execution_time_ms)
                    ),
                    fuel_used: Some(result.fuel_used),
                    memory_used_bytes: Some(result.memory_used_bytes),
                }),
            )
        }

        Ok(Err(error)) => {
            let status = match &error {
                SandboxError::EngineCreation(_)
                | SandboxError::StoreCreation(_) => {
                    StatusCode::INTERNAL_SERVER_ERROR
                }

                SandboxError::InvalidModule(_)
                | SandboxError::Instantiation(_)
                | SandboxError::InvalidContract(_) => {
                    StatusCode::BAD_REQUEST
                }
            };

            (
                status,
                Json(ExecuteResponse {
                    success: false,
                    message: error.to_string(),
                    output: Vec::new(),
                    execution_time_ms: None,
                    fuel_used: None,
                    memory_used_bytes: None,
                }),
            )
        }

        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ExecuteResponse {
                success: false,
                message: format!("Sandbox task failed: {}", error),
                output: Vec::new(),
                execution_time_ms: None,
                fuel_used: None,
                memory_used_bytes: None,
            }),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };

    use tower::ServiceExt;

    #[tokio::test]
    async fn health_endpoint_returns_ok() {
        let app = create_app();

        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app
            .oneshot(request)
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn execute_valid_guest_returns_ok() {
        let app = create_app();

        let body = r#"{
            "code": "(module (func (export \"run\") nop))"
        }"#;

        let request = Request::builder()
            .method("POST")
            .uri("/execute")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = app
            .oneshot(request)
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn execute_guest_without_run_returns_bad_request() {
        let app = create_app();

        let body = r#"{
            "code": "(module (func (export \"hello\")))"
        }"#;

        let request = Request::builder()
            .method("POST")
            .uri("/execute")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = app
            .oneshot(request)
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn execute_endpoint_returns_guest_output() {
        let app = create_app();

        let body = serde_json::json!({
            "code": r#"
                (module
                    (import "host" "print_number"
                        (func $print_number (param i32))
                    )

                    (func (export "run")
                        i32.const 42
                        call $print_number
                    )
                )
            "#
        })
        .to_string();

        let request = Request::builder()
            .method("POST")
            .uri("/execute")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = app
            .oneshot(request)
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::OK
        );

        let body = to_bytes(
            response.into_body(),
            usize::MAX,
        )
        .await
        .unwrap();

        let response: ExecuteResponse =
            serde_json::from_slice(&body).unwrap();

        assert!(response.success);

        assert_eq!(
            response.output,
            vec!["42".to_string()]
        );
    }

    #[tokio::test]
    async fn infinite_guest_returns_unprocessable_entity() {
        let app = create_app();

        let body = serde_json::json!({
            "code": r#"
                (module
                    (func (export "run")
                        (loop $forever
                            br $forever
                        )
                    )
                )
            "#
        })
        .to_string();

        let request = Request::builder()
            .method("POST")
            .uri("/execute")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = app
            .oneshot(request)
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }

    #[tokio::test]
    async fn oversized_guest_output_returns_unprocessable_entity() {
        let app = create_app();

        let body = serde_json::json!({
            "code": r#"
                (module
                    (import "host" "print_text"
                        (func $print_text (param i32 i32))
                    )

                    (memory (export "memory") 2)

                    (func (export "run")
                        i32.const 0
                        i32.const 65537
                        call $print_text
                    )
                )
            "#
        })
        .to_string();

        let request = Request::builder()
            .method("POST")
            .uri("/execute")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = app
            .oneshot(request)
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }

    #[tokio::test]
    async fn oversized_request_is_rejected() {
        let app = create_app();

        let oversized_code = "a".repeat(MAX_REQUEST_BYTES + 1);

        let body = serde_json::json!({
            "code": oversized_code
        })
        .to_string();

        let request = Request::builder()
            .method("POST")
            .uri("/execute")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = app
            .oneshot(request)
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::PAYLOAD_TOO_LARGE
        );
    }
}