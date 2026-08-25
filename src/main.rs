use std::env;
use wasmtime::{Config, Engine, Instance, Linker, Module, Caller, Store, StoreLimitsBuilder};
use std::time::Instant;

const MAX_FUEL: u64 = 10_000;
const MAX_MEMORY_BYTES: usize = 32 * 1024 * 1024; // 32 MB

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
        Some(instance) => instance,
        None => return Ok(()),
    };

    let run = match get_run_function(&instance, &mut store) {
        Some(run) => run,
        None => return Ok(()),
    };

    println!("Starting guest...");

    execute_guest(&run, &mut store);

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

fn create_engine() -> wasmtime::Result<Engine> {
    let mut config = Config::new();
    config.consume_fuel(true);

    Engine::new(&config)
}

fn create_store(engine: &Engine) -> wasmtime::Result<Store<wasmtime::StoreLimits>> {
    let limits = StoreLimitsBuilder::new()
        .memory_size(MAX_MEMORY_BYTES)
        .build();

    let mut store = Store::new(engine, limits);

    store.limiter(|limits| limits);
    store.set_fuel(MAX_FUEL)?;

    Ok(store)
}

fn instantiate_guest(
    engine: &Engine,
    store: &mut Store<wasmtime::StoreLimits>,
    module: &Module,
) -> Option<Instance> {
    let mut linker = Linker::new(engine);

    if let Err(error) = linker.func_wrap(
        "host",
        "print_text",
        |mut caller: Caller<'_, wasmtime::StoreLimits>, pointer: i32, length: i32|
                -> wasmtime::Result<()> {
            let memory = match caller.get_export("memory") {
                Some(wasmtime::Extern::Memory(memory)) => memory,

                _ => {
                    println!("Guest error: memory export not found.");
                    return Ok(());
                }
            };

            let data = memory.data(&caller);

            let start = match usize::try_from(pointer) {
                Ok(value) => value,
                Err(_) => {
                    return Err(wasmtime::Error::msg("invalid memory pointer"));
                }
            };

            let length = match usize::try_from(length) {
                Ok(value) => value,
                Err(_) => {
                    return Err(wasmtime::Error::msg("invalid text length"));
                }
            };

            let end = match start.checked_add(length) {
                Some(value) => value,
                None => {
                    return Err(wasmtime::Error::msg("invalid memory range"));
                }
            };

            if end > data.len() {
                return Err(wasmtime::Error::msg("memory access out of bounds"));
            }

            let text_bytes = &data[start..end];

            let text = match std::str::from_utf8(text_bytes) {
                Ok(text) => text,
                Err(_) => {
                    return Err(wasmtime::Error::msg("invalid UTF-8"));
                }
            };

            println!("Guest says: {}", text);
            Ok(())
        },
    ) {
        println!("Failed to create host function.");
        println!("Reason: {}", error);

        return None;
    }

    match linker.instantiate(&mut *store, module) {
        Ok(instance) => Some(instance),

        Err(error) => {
            let error_message = error.to_string();

            println!("Guest rejected!");

            if error_message.contains("memory") {
                println!("Reason: Guest exceeded the 32 MB memory limit.");
            } else {
                println!("Reason: {}", error_message);
            }

            None
        }
    }
}

fn get_run_function(
    instance: &Instance,
    store: &mut Store<wasmtime::StoreLimits>,
) -> Option<wasmtime::TypedFunc<(), ()>> {
    match instance.get_typed_func::<(), ()>(&mut *store, "run") {
        Ok(run) => Some(run),

        Err(_) => {
            println!("Guest rejected!");
            println!("Reason: Guest must export a function called run().");

            None
        }
    }
}

fn execute_guest(
    run: &wasmtime::TypedFunc<(), ()>,
    store: &mut Store<wasmtime::StoreLimits>,
) {
    let start = Instant::now();

    let execution_result = run.call(&mut *store, ());
    let duration = start.elapsed();

    println!("Execution time: {:.2} ms", duration.as_secs_f64() * 1000.0);

    match execution_result {
        Ok(_) => {
            println!("Guest finished successfully.");
        }

        Err(error) => {
            println!("Guest was stopped!");

            let error_message = format!("{:#}", error);

            if error_message.contains("fuel") {
                println!("Reason: Execution limit exceeded.");
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