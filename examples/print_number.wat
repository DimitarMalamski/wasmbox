(module
    (import "host" "print_number" (func $print_number (param i32)))

    (func (export "run")
        i32.const 42
        call $print_number
    )
)