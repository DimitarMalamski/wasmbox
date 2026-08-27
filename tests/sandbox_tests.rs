use wasmbox::sandbox::{
    execute_wat,
    ExecutionError,
    SandboxError,
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

    let result = execute_wat(code)
        .expect("Sandbox setup should succeed");

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

    let result = execute_wat(code)
        .expect("Sandbox setup should succeed");

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

    assert!(matches!(
        result,
        Err(SandboxError::InvalidContract(_))
    ));
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

    let result = execute_wat(code)
        .expect("Sandbox setup should succeed");

    assert!(result.success);

    assert_eq!(
        result.output,
        vec!["42".to_string()]
    );
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

    let result = execute_wat(code)
        .expect("Sandbox setup should succeed");

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

    assert!(matches!(
        result,
        Err(SandboxError::Instantiation(_))
    ));
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

    let result = execute_wat(code)
        .expect("Sandbox setup should succeed");

    assert!(!result.success);

    assert!(matches!(
        result.error,
        Some(ExecutionError::InvalidUtf8)
    ));
}