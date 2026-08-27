use wasmbox::sandbox::{
    create_engine,
    create_store,
    execute_run,
    get_run_function,
    instantiate_guest,
    SandboxState,
    MAX_FUEL,
    MAX_MEMORY_BYTES,
};

use std::env;

use wasmtime::{
    Engine, Instance, Module, Store,
};
fn main() -> wasmtime::Result<()> {
    let engine = create_engine()?;

    let guest_path = match get_guest_path() {
        Some(path) => path,
        None => return Ok(()),
    };

    println!("Loading guest: {}", guest_path);

    let module = match load_guest(&engine, &guest_path) {
        Some(module) => module,
        None => return Ok(()),
    };

    let mut store = create_store(&engine)?;

    let instance = match instantiate_guest(&engine, &mut store, &module) {
        Ok(instance) => instance,

        Err(error) => {
            let error_message = format!("{:#}", error);

            println!("Guest rejected!");

            if error_message.contains("memory") {
                println!("Reason: Guest exceeded the 32 MB memory limit.");
            } else {
                println!("Reason: {}", error_message);
            }

            return Ok(());
        }
    };

    let run = match get_run_function(&instance, &mut store) {
        Ok(run) => run,

        Err(error) => {
            println!("Guest rejected!");
            println!("Reason: {}", error);

            return Ok(());
        }
    };

    println!("Starting guest...");

    execute_guest(&engine, &instance, &run, &mut store);

    Ok(())
}

fn load_guest(engine: &Engine, guest_path: &str) -> Option<Module> {

    if !std::path::Path::new(guest_path).exists() {
        println!("Reason: File does not exist.");
        return None;
    }

    match Module::from_file(engine, guest_path) {
        Ok(module) => Some(module),
        
        Err(error) => {
            println!("Reason: The guest is not valid WebAssembly.");
            println!("Details: {}", error);

            None
        }
    }
}

fn execute_guest(
    engine: &Engine,
    instance: &Instance,
    run: &wasmtime::TypedFunc<(), ()>,
    store: &mut Store<SandboxState>,
) {
    let result = execute_run(
        engine,
        instance,
        run,
        store,
    );

    for line in &result.output {
        println!("Guest says: {}", line);
    }

    println!(
        "Execution time: {:.2} ms",
        result.execution_time_ms
    );

    println!(
        "Fuel used: {} / {}",
        result.fuel_used,
        MAX_FUEL
    );

    println!(
        "Memory allocated: {:.2} KB / {:.2} MB",
        result.memory_used_bytes as f64 / 1024.0,
        MAX_MEMORY_BYTES as f64 / (1024.0 * 1024.0)
    );

    if result.success {
        println!("Guest finished successfully.");
    } else {
        println!("Guest was stopped!");
        println!("Reason: {}", result.message);
    }
}

fn get_guest_path() -> Option<String> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Usage: cargo run -- <path-to-wat-file>");
        return None;
    }

    Some(args[1].clone())
}