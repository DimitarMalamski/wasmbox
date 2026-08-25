use wasmtime::{Engine, Linker, Module, Store};

fn main() -> wasmtime::Result<()> {
    // Create the WebAssembly runtime
    let engine = Engine::default();

    // Load WebAssembly code from an external file
    let module = Module::from_file(&engine, "examples/add.wat")?;

    // Create the environment where the guest will run
    let mut store = Store::new(&engine, ());

    // Prepare the module for execution
    let linker = Linker::new(&engine);
    let instance = linker.instantiate(&mut store, &module)?;

    // Find the exported "add" function
    let add = instance.get_typed_func::<(i32, i32), i32>(
        &mut store,
        "add",
    )?;

    // Execute the guest function
    let result = add.call(&mut store, (10, 20))?;

    println!("Guest returned: {}", result);

    Ok(())
}