pub const MAX_FUEL: u64 = 10_000;
pub const MAX_MEMORY_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_EXECUTION_TIME_SECONDS: u64 = 2;
pub const MAX_OUTPUT_BYTES: usize = 64 * 1024;

pub const MAX_ALLOWED_FUEL: u64 = 10_000_000;
pub const MAX_ALLOWED_MEMORY_BYTES: usize = 256 * 1024 * 1024;
pub const MAX_ALLOWED_EXECUTION_TIME_SECONDS: u64 = 30;
pub const MAX_ALLOWED_OUTPUT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct SandboxConfig {
    pub max_fuel: u64,
    pub max_memory_bytes: usize,
    pub max_execution_time_seconds: u64,
    pub max_output_bytes: usize,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            max_fuel: MAX_FUEL,
            max_memory_bytes: MAX_MEMORY_BYTES,
            max_execution_time_seconds: MAX_EXECUTION_TIME_SECONDS,
            max_output_bytes: MAX_OUTPUT_BYTES,
        }
    }
}

pub fn validate_sandbox_config(config: SandboxConfig) -> Result<SandboxConfig, String> {
    if config.max_fuel == 0 {
        return Err("WASMBOX_MAX_FUEL must be greater than 0.".to_string());
    }

    if config.max_memory_bytes == 0 {
        return Err("WASMBOX_MAX_MEMORY_BYTES must be greater than 0.".to_string());
    }

    if config.max_execution_time_seconds == 0 {
        return Err("WASMBOX_MAX_EXECUTION_TIME_SECONDS must be greater than 0.".to_string());
    }

    if config.max_output_bytes == 0 {
        return Err("WASMBOX_MAX_OUTPUT_BYTES must be greater than 0.".to_string());
    }

    if config.max_fuel > MAX_ALLOWED_FUEL {
        return Err(format!(
            "WASMBOX_MAX_FUEL cannot exceed {}.",
            MAX_ALLOWED_FUEL
        ));
    }

    if config.max_memory_bytes > MAX_ALLOWED_MEMORY_BYTES {
        return Err(format!(
            "WASMBOX_MAX_MEMORY_BYTES cannot exceed {}.",
            MAX_ALLOWED_MEMORY_BYTES
        ));
    }

    if config.max_execution_time_seconds > MAX_ALLOWED_EXECUTION_TIME_SECONDS {
        return Err(format!(
            "WASMBOX_MAX_EXECUTION_TIME_SECONDS cannot exceed {}.",
            MAX_ALLOWED_EXECUTION_TIME_SECONDS
        ));
    }

    if config.max_output_bytes > MAX_ALLOWED_OUTPUT_BYTES {
        return Err(format!(
            "WASMBOX_MAX_OUTPUT_BYTES cannot exceed {}.",
            MAX_ALLOWED_OUTPUT_BYTES
        ));
    }

    Ok(config)
}
