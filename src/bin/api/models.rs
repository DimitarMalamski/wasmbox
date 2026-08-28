use serde::{Deserialize, Serialize};

use wasmbox::sandbox::SandboxResult;

#[derive(Deserialize)]
pub struct ExecuteRequest {
    pub code: String,
}

#[derive(Serialize, Deserialize)]
pub struct ExecuteResponse {
    pub success: bool,
    pub message: String,
    pub output: Vec<String>,
    pub execution_time_ms: Option<f64>,
    pub fuel_used: Option<u64>,
    pub memory_used_bytes: Option<usize>,
}

impl ExecuteResponse {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            output: Vec::new(),
            execution_time_ms: None,
            fuel_used: None,
            memory_used_bytes: None,
        }
    }

    pub fn from_result(result: SandboxResult) -> Self {
        Self {
            success: result.success,
            message: result.message,
            output: result.output,
            execution_time_ms: Some(round_to_two_decimals(result.execution_time_ms)),
            fuel_used: Some(result.fuel_used),
            memory_used_bytes: Some(result.memory_used_bytes),
        }
    }
}

fn round_to_two_decimals(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_response_carries_no_metrics() {
        let response = ExecuteResponse::error("nope");

        assert!(!response.success);
        assert_eq!(response.message, "nope");
        assert!(response.output.is_empty());
        assert!(response.execution_time_ms.is_none());
        assert!(response.fuel_used.is_none());
        assert!(response.memory_used_bytes.is_none());
    }

    #[test]
    fn result_response_rounds_execution_time() {
        let result = SandboxResult {
            success: true,
            message: "ok".to_string(),
            error: None,
            output: Vec::new(),
            execution_time_ms: 0.193_49,
            fuel_used: 7,
            memory_used_bytes: 64,
        };

        let response = ExecuteResponse::from_result(result);

        let rounded = response.execution_time_ms.expect("time is present");

        assert!((rounded - 0.19).abs() < f64::EPSILON);
        assert_eq!(response.fuel_used, Some(7));
    }
}
