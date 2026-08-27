use std::env;

use wasmbox::sandbox::{SandboxConfig, validate_sandbox_config};

pub(super) fn load_sandbox_config() -> Result<SandboxConfig, String> {
    dotenvy::dotenv().ok();

    let default = SandboxConfig::default();

    let config = SandboxConfig {
        max_fuel: parse_env_value("WASMBOX_MAX_FUEL", default.max_fuel)?,

        max_memory_bytes: parse_env_value("WASMBOX_MAX_MEMORY_BYTES", default.max_memory_bytes)?,

        max_execution_time_seconds: parse_env_value(
            "WASMBOX_MAX_EXECUTION_TIME_SECONDS",
            default.max_execution_time_seconds,
        )?,

        max_output_bytes: parse_env_value("WASMBOX_MAX_OUTPUT_BYTES", default.max_output_bytes)?,
    };

    validate_sandbox_config(config)
}

fn parse_config_value<T>(name: &str, value: &str) -> Result<T, String>
where
    T: std::str::FromStr,
{
    value
        .parse::<T>()
        .map_err(|_| format!("Invalid value for environment variable {}: {}", name, value))
}

fn parse_env_value<T>(name: &str, default: T) -> Result<T, String>
where
    T: std::str::FromStr,
{
    match env::var(name) {
        Ok(value) => parse_config_value(name, &value),

        Err(env::VarError::NotPresent) => Ok(default),

        Err(error) => Err(format!(
            "Failed to read environment variable {}: {}",
            name, error
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_environment_value_is_parsed() {
        let result =
            parse_config_value::<u64>("WASMBOX_MAX_FUEL", "20000").expect("Value should be valid");

        assert_eq!(result, 20_000);
    }

    #[test]
    fn invalid_environment_value_is_rejected() {
        let result = parse_config_value::<u64>("WASMBOX_MAX_FUEL", "banana");

        assert!(result.is_err());

        assert_eq!(
            result.unwrap_err(),
            "Invalid value for environment variable WASMBOX_MAX_FUEL: banana"
        );
    }
}
