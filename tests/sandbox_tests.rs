use wasmbox::sandbox::execute_wat;

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