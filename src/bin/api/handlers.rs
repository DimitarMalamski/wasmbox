use axum::{Json, extract::State, http::StatusCode};

use wasmbox::sandbox::{ExecutionError, SandboxError, execute_wat_with_config};

use super::{
    models::{ExecuteRequest, ExecuteResponse},
    state::AppState,
};

pub(super) async fn health() -> &'static str {
    "WasmBox is running"
}

fn execution_status(error: &ExecutionError) -> StatusCode {
    match error {
        ExecutionError::Timeout => StatusCode::REQUEST_TIMEOUT,

        ExecutionError::FuelExhausted
        | ExecutionError::InvalidMemoryAccess
        | ExecutionError::InvalidPointer
        | ExecutionError::InvalidTextLength
        | ExecutionError::InvalidMemoryRange
        | ExecutionError::InvalidUtf8
        | ExecutionError::OutputLimitExceeded
        | ExecutionError::Other(_) => StatusCode::UNPROCESSABLE_ENTITY,
    }
}

fn sandbox_status(error: &SandboxError) -> StatusCode {
    match error {
        SandboxError::EngineCreation(_)
        | SandboxError::StoreCreation(_)
        | SandboxError::InvalidConfig(_) => StatusCode::INTERNAL_SERVER_ERROR,

        SandboxError::SourceTooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,

        SandboxError::InvalidModule(_)
        | SandboxError::Instantiation(_)
        | SandboxError::InvalidContract(_) => StatusCode::BAD_REQUEST,
    }
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
                Json(ExecuteResponse::error(
                    "Server is busy. Too many concurrent executions.",
                )),
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
                Some(error) => execution_status(error),
            };

            (status, Json(ExecuteResponse::from_result(result)))
        }

        Ok(Err(error)) => (
            sandbox_status(&error),
            Json(ExecuteResponse::error(error.to_string())),
        ),

        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ExecuteResponse::error(format!(
                "Sandbox task failed: {}",
                error
            ))),
        ),
    }
}
