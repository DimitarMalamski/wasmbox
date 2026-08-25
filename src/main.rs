use wasmtime::{Config, Engine, Linker, Module, Store, StoreLimitsBuilder};

fn main() -> wasmtime::Result<()> {
    // Create Wasmtime configuration
    let mut config = Config::new();

    // Enable fuel consumption
    config.consume_fuel(true);

    // Create the engine using our configuration
    let engine = Engine::new(&config)?;

    // Load our infinite-loop guest
    let module = Module::from_file(&engine, "examples/infinite.wat")?;

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

    let instance = match linker.instantiate(&mut store, &module) {
        Ok(instance) => instance,

        Err(error) => {
            println!("Guest rejected!");

            let error_message = error.to_string();

            if error_message.contains("memory") {
                println!("Reason: Guest exceeded the 32 MB memory limit.");
            } else {
                println!("Reason: {}", error_message);
            }

            return Ok(());
        }
    };

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

            let error_message = format!("{:#}", error);

            if error_message.contains("fuel") {
                println!("Reason: Execution limit exceeded.");
            } else {
                println!("Reason: {}", error_message);
            }
        }
    }

    Ok(())
}