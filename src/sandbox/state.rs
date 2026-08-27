use wasmtime::StoreLimits;

use super::config::SandboxConfig;
use super::error::ExecutionError;

pub struct SandboxState {
    pub limits: StoreLimits,
    pub output: Vec<String>,
    pub output_bytes: usize,
    pub config: SandboxConfig,
}

pub struct SandboxResult {
    pub success: bool,
    pub message: String,
    pub error: Option<ExecutionError>,
    pub output: Vec<String>,
    pub execution_time_ms: f64,
    pub fuel_used: u64,
    pub memory_used_bytes: usize,
}
