use wasmbox::sandbox::{
    create_engine,
    create_store,
    get_run_function,
    instantiate_guest,
    MAX_EXECUTION_TIME_SECONDS,
    MAX_FUEL,
    MAX_MEMORY_BYTES,
};

use std::env;

use wasmtime::{
    Engine, Instance, Module, Store,
};

use std::time::{Duration, Instant};
use std::thread;
use std::sync::mpsc;
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
    store: &mut Store<wasmtime::StoreLimits>,
) {
    let (cancel_sender, cancel_receiver) = mpsc::channel::<()>();

    let timeout_engine = engine.clone();

    let timeout_handle = thread::spawn(move || {
        if cancel_receiver
            .recv_timeout(Duration::from_secs(MAX_EXECUTION_TIME_SECONDS))
            .is_err()
        {
            timeout_engine.increment_epoch();
        }
    });

    let start = Instant::now();

    let fuel_before = store.get_fuel().unwrap_or(0);

    let execution_result = run.call(&mut *store, ());

    let guest_memory = instance.get_memory(&mut *store, "memory");
    let memory_used_bytes = match guest_memory {
        Some(memory) => memory.data_size(&*store),
        None => 0,
    };

    let remaining_fuel = store.get_fuel().unwrap_or(0);
    let fuel_used = fuel_before.saturating_sub(remaining_fuel);

    let _ = cancel_sender.send(());
    let _ = timeout_handle.join();

    let duration = start.elapsed();

    println!("Execution time: {:.2} ms", duration.as_secs_f64() * 1000.0);
    println!("Fuel used: {} / {}", fuel_used, MAX_FUEL);

    println!(
        "Memory allocated: {:.2} KB / {:.2} MB",
        memory_used_bytes as f64 / 1024.0,
        MAX_MEMORY_BYTES as f64 / (1024.0 * 1024.0)
    );

    match execution_result {
        Ok(_) => {
            println!("Guest finished successfully.");
        }

        Err(error) => {
            println!("Guest was stopped!");

            let error_message = format!("{:#}", error);

            if error_message.contains("fuel") {
                println!("Reason: Execution limit exceeded.");
            } else if error_message.contains("wasm trap: interrupt") {
                println!("Reason: Maximum execution time exceeded.");
            } else if error_message.contains("memory access out of bounds") {
                println!("Reason: Guest attempted invalid memory access.");
            } else if error_message.contains("invalid memory pointer") {
                println!("Reason: Guest provided an invalid memory pointer.");
            } else if error_message.contains("invalid text length") {
                println!("Reason: Guest provided an invalid text length.");
            } else if error_message.contains("invalid memory range") {
                println!("Reason: Guest provided an invalid memory range.");
            } else if error_message.contains("invalid UTF-8") {
                println!("Reason: Guest provided invalid UTF-8 text.");
            } else {
                println!("Reason: {}", error_message);
            }
        }
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