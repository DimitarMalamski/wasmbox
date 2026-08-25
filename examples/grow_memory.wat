(module
    (memory (export "memory") 1)

    (func (export "run")
        i32.const 10
        memory.grow
        drop
    )
)