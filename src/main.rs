use wasmtime::{Config, Engine, Linker, Module, Store, StoreLimitsBuilder};

fn main() -> wasmtime::Result<()> {
    // Create Wasmtime configuration
    let mut config = Config::new();

    // Enable fuel consumption
    config.consume_fuel(true);

    // Create the engine using our configuration
    let engine = Engine::new(&config)?;

    // Load our infinite-loop guest
    let module = Module::from_file(&engine, "examples/memory_hog.wat")?;

    let limits = StoreLimitsBuilder::new()
        .memory_size(32 * 1024 * 1024)
        .build();

    // Create the guest environment
    let mut store = Store::new(&engine, limits);
    
    store.limiter(|limits| limits);

    // Give the guest 10,000 fuel
    store.set_fuel(10_000)?;

    // Start the WebAssembly module
    let linker = Linker::new(&engine);
    let instance = linker.instantiate(&mut store, &module)?;

    // Find the "run" function
    let run = instance.get_typed_func::<(), ()>(
        &mut store,
        "run",
    )?;

    println!("Starting guest...");

    // Try running it
    match run.call(&mut store, ()) {
        Ok(_) => {
            println!("Guest finished successfully.");
        }

        Err(error) => {
            println!("Guest was stopped!");
            println!("Reason: {}", error);
        }
    }

    Ok(())
}