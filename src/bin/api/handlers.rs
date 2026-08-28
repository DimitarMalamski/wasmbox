use axum::{Json, extract::State, http::StatusCode};

use wasmbox::sandbox::{ExecutionError, SandboxError, execute_wat_with_config};

use super::{
    models::{ExecuteRequest, ExecuteResponse},
    state::AppState,
};

pub(super) async fn health() -> &'static str {
    "WasmBox is running"
}

fn round_to_two_decimals(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

pub(super) async fn execute(
    State(state): State<AppState>,
    Json(request): Json<ExecuteRequest>,
) -> (StatusCode, Json<ExecuteResponse>) {
    let permit = match state.execution_semaphore.clone().try_acquire_owned() {
        Ok(permit) => permit,

        Err(_) => {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(ExecuteResponse {
                    success: false,
                    message: "Server is busy. Too many concurrent executions.".to_string(),
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

        execute_wat_with_config(&code, sandbox_config)
    })
    .await;

    match execution {
        Ok(Ok(result)) => {
            let status = match &result.error {
                None => StatusCode::OK,

                Some(ExecutionError::Timeout) => StatusCode::REQUEST_TIMEOUT,

                Some(
                    ExecutionError::FuelExhausted
                    | ExecutionError::InvalidMemoryAccess
                    | ExecutionError::InvalidPointer
                    | ExecutionError::InvalidTextLength
                    | ExecutionError::InvalidMemoryRange
                    | ExecutionError::InvalidUtf8
                    | ExecutionError::OutputLimitExceeded
                    | ExecutionError::Other(_),
                ) => StatusCode::UNPROCESSABLE_ENTITY,
            };

            (
                status,
                Json(ExecuteResponse {
                    success: result.success,
                    message: result.message,
                    output: result.output,
                    execution_time_ms: Some(round_to_two_decimals(result.execution_time_ms)),
                    fuel_used: Some(result.fuel_used),
                    memory_used_bytes: Some(result.memory_used_bytes),
                }),
            )
        }

        Ok(Err(error)) => {
            let status = match &error {
                SandboxError::EngineCreation(_)
                | SandboxError::StoreCreation(_)
                | SandboxError::InvalidConfig(_) => StatusCode::INTERNAL_SERVER_ERROR,

                SandboxError::SourceTooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,

                SandboxError::InvalidModule(_)
                | SandboxError::Instantiation(_)
                | SandboxError::InvalidContract(_) => StatusCode::BAD_REQUEST,
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
