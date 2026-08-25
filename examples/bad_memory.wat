(module
    (import "host" "print_text" (func $print_text (param i32 i32)))

    (memory (export "memory") 1)

    (func (export "run")
        i32.const 1000000
        i32.const 50
        call $print_text
    )
)