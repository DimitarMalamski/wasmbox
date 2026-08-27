use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use wasmtime::{
    Caller, Config, Engine, Instance, Linker, Module, Store, StoreLimits, StoreLimitsBuilder, TypedFunc,
};

pub const MAX_FUEL: u64 = 10_000;
pub const MAX_MEMORY_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_EXECUTION_TIME_SECONDS: u64 = 2;

pub struct SandboxState {
    pub limits: StoreLimits,
    pub output: Vec<String>,
}
pub struct SandboxResult {
    pub success: bool,
    pub message: String,
    pub output: Vec<String>,
    pub execution_time_ms: f64,
    pub fuel_used: u64,
    pub memory_used_bytes: usize,
}

pub fn create_engine() -> wasmtime::Result<Engine> {
    let mut config = Config::new();

    config.consume_fuel(true);
    config.epoch_interruption(true);

    Engine::new(&config)
}

pub fn create_store(
    engine: &Engine,
) -> wasmtime::Result<Store<SandboxState>> {
    let limits = StoreLimitsBuilder::new()
        .memory_size(MAX_MEMORY_BYTES)
        .build();

    let state = SandboxState {
        limits,
        output: Vec::new(),
    };

    let mut store = Store::new(engine, state);

    store.limiter(|state| &mut state.limits);
    store.set_fuel(MAX_FUEL)?;
    store.set_epoch_deadline(1);

    Ok(store)
}

pub fn friendly_execution_error(error: &wasmtime::Error) -> String {
    let error_message = format!("{:#}", error);

    if error_message.contains("fuel") {
        "Execution limit exceeded.".to_string()
    } else if error_message.contains("wasm trap: interrupt") {
        "Maximum execution time exceeded.".to_string()
    } else if error_message.contains("memory access out of bounds") {
        "Guest attempted invalid memory access.".to_string()
    } else if error_message.contains("invalid memory pointer") {
        "Guest provided an invalid memory pointer.".to_string()
    } else if error_message.contains("invalid text length") {
        "Guest provided an invalid text length.".to_string()
    } else if error_message.contains("invalid memory range") {
        "Guest provided an invalid memory range.".to_string()
    } else if error_message.contains("invalid UTF-8") {
        "Guest provided invalid UTF-8 text.".to_string()
    } else {
        error_message
    }
}

pub fn execute_run(
    engine: &Engine,
    instance: &Instance,
    run: &TypedFunc<(), ()>,
    store: &mut Store<SandboxState>,
) -> SandboxResult {
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

    let remaining_fuel = store.get_fuel().unwrap_or(0);
    let fuel_used = fuel_before.saturating_sub(remaining_fuel);

    let guest_memory = instance.get_memory(&mut *store, "memory");

    let memory_used_bytes = match guest_memory {
        Some(memory) => memory.data_size(&*store),
        None => 0,
    };

    let output = store.data().output.clone();

    let _ = cancel_sender.send(());
    let _ = timeout_handle.join();

    let execution_time_ms = start.elapsed().as_secs_f64() * 1000.0;

    match execution_result {
        Ok(_) => SandboxResult {
            success: true,
            message: "Guest executed successfully.".to_string(),
            output: output.clone(),
            execution_time_ms,
            fuel_used,
            memory_used_bytes,
        },

        Err(error) => SandboxResult {
            success: false,
            message: friendly_execution_error(&error),
            output,
            execution_time_ms,
            fuel_used,
            memory_used_bytes,
        },
    }
}

pub fn execute_wat(code: &str) -> Result<SandboxResult, String> {
    let engine = create_engine()
        .map_err(|error| {
            format!("Failed to create sandbox: {}", error)
        })?;

    let module = Module::new(&engine, code)
        .map_err(|error| {
            format!("Invalid WebAssembly: {}", error)
        })?;

    let mut store = create_store(&engine)
        .map_err(|error| {
            format!("Failed to create sandbox store: {}", error)
        })?;

    let instance = instantiate_guest(
        &engine,
        &mut store,
        &module,
    )
    .map_err(|error| {
        format!("Could not instantiate guest: {}", error)
    })?;

    let run = get_run_function(
        &instance,
        &mut store,
    )
    .map_err(|error| error.to_string())?;

    Ok(execute_run(
        &engine,
        &instance,
        &run,
        &mut store,
    ))
}

pub fn register_print_number(
    linker: &mut Linker<SandboxState>,
) -> wasmtime::Result<()> {
    linker.func_wrap(
        "host",
        "print_number",
        |mut caller: Caller<'_, SandboxState>, number: i32| {

            caller
                .data_mut()
                .output
                .push(number.to_string());
        },
    )?;

    Ok(())
}

pub fn register_print_text(
    linker: &mut Linker<SandboxState>,
) -> wasmtime::Result<()> {
    linker.func_wrap("host", "print_text", host_print_text)?;

    Ok(())
}

fn host_print_text(
    mut caller: Caller<'_, SandboxState>,
    pointer: i32,
    length: i32,
) -> wasmtime::Result<()> {
    let memory = caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
        .ok_or_else(|| wasmtime::Error::msg("memory export not found"))?;

    let data = memory.data(&caller);

    let start = usize::try_from(pointer)
        .map_err(|_| wasmtime::Error::msg("invalid memory pointer"))?;

    let length = usize::try_from(length)
        .map_err(|_| wasmtime::Error::msg("invalid text length"))?;

    let end = start
        .checked_add(length)
        .ok_or_else(|| wasmtime::Error::msg("invalid memory range"))?;

    if end > data.len() {
        return Err(wasmtime::Error::msg(
            "memory access out of bounds",
        ));
    }

    let bytes = &data[start..end];

    let text = std::str::from_utf8(bytes)
        .map_err(|_| wasmtime::Error::msg("invalid UTF-8"))?
        .to_string();

    caller.data_mut().output.push(text);

    Ok(())
}

pub fn register_host_functions(
    linker: &mut Linker<SandboxState>,
) -> wasmtime::Result<()> {
    register_print_number(linker)?;
    register_print_text(linker)?;

    Ok(())
}

pub fn instantiate_guest(
    engine: &Engine,
    store: &mut Store<SandboxState>,
    module: &Module,
) -> wasmtime::Result<Instance> {
    let mut linker = Linker::new(engine);

    register_host_functions(&mut linker)?;

    linker.instantiate(store, module)
}

pub fn get_run_function(
    instance: &Instance,
    store: &mut Store<SandboxState>,
) -> wasmtime::Result<TypedFunc<(), ()>> {
    instance
        .get_typed_func::<(), ()>(store, "run")
        .map_err(|_| wasmtime::Error::msg("Guest must export run()."))
}