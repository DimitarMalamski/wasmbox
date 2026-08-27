use serde::{Deserialize, Serialize};

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
