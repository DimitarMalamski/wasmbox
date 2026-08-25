(module
    (import "host" "print_text" (func $print_text (param i32 i32)))

    (memory (export "memory") 1)

    (data (i32.const 0) "\ff\fe")

    (func (export "run")
        i32.const 0
        i32.const 2
        call $print_text
    )
)