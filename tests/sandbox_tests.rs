use wasmbox::sandbox::{
    ExecutionError, MAX_ALLOWED_MEMORY_BYTES, MAX_OUTPUT_BYTES, SandboxConfig, SandboxError,
    execute_wat, execute_wat_with_config, validate_sandbox_config,
};

#[test]
fn safe_guest_executes_successfully() {
    let code = r#"
        (module
            (func (export "run")
                nop
            )
        )
    "#;

    let result = execute_wat(code).expect("Sandbox setup should succeed");

    assert!(result.success);
    assert_eq!(result.message, "Guest executed successfully.");
    assert_eq!(result.fuel_used, 1);
}

#[test]
fn infinite_guest_is_stopped_by_fuel_limit() {
    let code = r#"
        (module
            (func (export "run")
                (loop $forever
                    br $forever
                )
            )
        )
    "#;

    let result = execute_wat(code).expect("Sandbox setup should succeed");

    assert!(!result.success);
    assert_eq!(result.message, "Execution limit exceeded.");
    assert_eq!(result.fuel_used, 10_000);
}

#[test]
fn guest_without_run_function_is_rejected() {
    let code = r#"
        (module
            (func (export "hello"))
        )
    "#;

    let result = execute_wat(code);

    assert!(matches!(result, Err(SandboxError::InvalidContract(_))));
}

#[test]
fn guest_output_is_captured() {
    let code = r#"
        (module
            (import "host" "print_number"
                (func $print_number (param i32))
            )

            (func (export "run")
                i32.const 42
                call $print_number
            )
        )
    "#;

    let result = execute_wat(code).expect("Sandbox setup should succeed");

    assert!(result.success);

    assert_eq!(result.output, vec!["42".to_string()]);
}

#[test]
fn invalid_memory_access_is_rejected() {
    let code = r#"
        (module
            (import "host" "print_text"
                (func $print_text (param i32 i32))
            )

            (memory (export "memory") 1)

            (func (export "run")
                i32.const 1000000
                i32.const 50
                call $print_text
            )
        )
    "#;

    let result = execute_wat(code).expect("Sandbox setup should succeed");

    assert!(!result.success);

    assert!(matches!(
        result.error,
        Some(ExecutionError::InvalidMemoryAccess)
    ));
}

#[test]
fn guest_exceeding_memory_limit_is_rejected() {
    let code = r#"
        (module
            (memory 600)
            (func (export "run"))
        )
    "#;

    let result = execute_wat(code);

    assert!(matches!(result, Err(SandboxError::Instantiation(_))));
}

#[test]
fn invalid_utf8_is_rejected() {
    let code = r#"
        (module
            (import "host" "print_text"
                (func $print_text (param i32 i32))
            )

            (memory (export "memory") 1)

            (data (i32.const 0) "\ff\fe")

            (func (export "run")
                i32.const 0
                i32.const 2
                call $print_text
            )
        )
    "#;

    let result = execute_wat(code).expect("Sandbox setup should succeed");

    assert!(!result.success);

    assert!(matches!(result.error, Some(ExecutionError::InvalidUtf8)));
}

#[test]
fn guest_exceeding_output_limit_is_stopped() {
    let output_size = MAX_OUTPUT_BYTES + 1;

    let code = format!(
        r#"
        (module
            (import "host" "print_text"
                (func $print_text (param i32 i32))
            )

            (memory (export "memory") 2)

            (func (export "run")
                i32.const 0
                i32.const {}
                call $print_text
            )
        )
        "#,
        output_size
    );

    let result = execute_wat(&code).expect("Sandbox setup should succeed");

    assert!(!result.success);

    assert!(matches!(
        result.error,
        Some(ExecutionError::OutputLimitExceeded)
    ));
}

#[test]
fn timeout_thread_does_not_interfere_with_fast_guest() {
    let code = r#"
        (module
            (import "host" "print_number"
                (func $print_number (param i32))
            )
            (func (export "run")
                i32.const 7
                call $print_number
            )
        )
    "#;

    let config = SandboxConfig {
        max_execution_time_seconds: 1,
        ..Default::default()
    };

    let result = execute_wat_with_config(code, config).expect("Sandbox setup should succeed");

    assert!(result.success);
    assert_eq!(result.output, vec!["7".to_string()]);
    assert!(result.execution_time_ms < 1000.0);
}

#[test]
fn zero_fuel_configuration_is_rejected() {
    let config = SandboxConfig {
        max_fuel: 0,
        ..Default::default()
    };

    let result = validate_sandbox_config(config);

    assert!(result.is_err());

    assert_eq!(
        result.unwrap_err(),
        "WASMBOX_MAX_FUEL must be greater than 0."
    );
}

#[test]
fn excessive_memory_configuration_is_rejected() {
    let config = SandboxConfig {
        max_memory_bytes: MAX_ALLOWED_MEMORY_BYTES + 1,
        ..Default::default()
    };

    let result = validate_sandbox_config(config);

    assert!(result.is_err());

    assert_eq!(
        result.unwrap_err(),
        format!(
            "WASMBOX_MAX_MEMORY_BYTES cannot exceed {}.",
            MAX_ALLOWED_MEMORY_BYTES
        )
    );
}

#[test]
fn guest_declaring_multiple_memories_is_rejected() {
    let code = r#"
        (module
            (memory 1)
            (memory 1)
            (func (export "run"))
        )
    "#;

    let result = execute_wat(code);

    assert!(matches!(result, Err(SandboxError::InvalidModule(_))));
}

#[test]
fn guest_declaring_an_oversized_table_is_rejected() {
    let code = r#"
        (module
            (table 10000000 funcref)
            (func (export "run"))
        )
    "#;

    let result = execute_wat(code);

    assert!(matches!(result, Err(SandboxError::Instantiation(_))));
}

#[test]
fn execution_rejects_zero_fuel_config() {
    let code = r#"
        (module
            (func (export "run"))
        )
    "#;

    let config = SandboxConfig {
        max_fuel: 0,
        ..Default::default()
    };

    let result = execute_wat_with_config(code, config);

    assert!(matches!(result, Err(SandboxError::InvalidConfig(_))));
}

#[test]
fn execution_rejects_config_above_allowed_maximum() {
    let code = r#"
        (module
            (func (export "run"))
        )
    "#;

    let config = SandboxConfig {
        max_memory_bytes: MAX_ALLOWED_MEMORY_BYTES + 1,
        ..Default::default()
    };

    let result = execute_wat_with_config(code, config);

    assert!(matches!(result, Err(SandboxError::InvalidConfig(_))));
}

#[test]
fn oversized_guest_source_is_rejected() {
    let filler = " ".repeat(300 * 1024);
    let code = format!("(module {} (func (export \"run\")))", filler);

    let result = execute_wat(&code);

    assert!(matches!(result, Err(SandboxError::SourceTooLarge(_))));
}

#[test]
fn guest_with_infinite_start_function_is_rejected() {
    let code = r#"
        (module
            (func $spin
                (loop $forever
                    br $forever
                )
            )
            (start $spin)
            (func (export "run"))
        )
    "#;

    let result = execute_wat(code);

    assert!(matches!(result, Err(SandboxError::Instantiation(_))));
}
