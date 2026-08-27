mod config;
mod error;
mod host;
mod state;

pub use config::{
    MAX_ALLOWED_EXECUTION_TIME_SECONDS, MAX_ALLOWED_FUEL, MAX_ALLOWED_MEMORY_BYTES,
    MAX_ALLOWED_OUTPUT_BYTES, MAX_EXECUTION_TIME_SECONDS, MAX_FUEL, MAX_MEMORY_BYTES,
    MAX_OUTPUT_BYTES, SandboxConfig, validate_sandbox_config,
};

pub use error::{ExecutionError, SandboxError};

pub use state::{SandboxResult, SandboxState};

use self::host::register_host_functions;

use self::error::classify_execution_error;

use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use wasmtime::{Config, Engine, Instance, Linker, Module, Store, StoreLimitsBuilder, TypedFunc};

pub fn create_engine() -> wasmtime::Result<Engine> {
    let mut config = Config::new();

    config.consume_fuel(true);
    config.epoch_interruption(true);

    Engine::new(&config)
}

pub fn create_store(engine: &Engine) -> wasmtime::Result<Store<SandboxState>> {
    create_store_with_config(engine, SandboxConfig::default())
}

pub fn create_store_with_config(
    engine: &Engine,
    config: SandboxConfig,
) -> wasmtime::Result<Store<SandboxState>> {
    let limits = StoreLimitsBuilder::new()
        .memory_size(config.max_memory_bytes)
        .build();

    let mut store = Store::new(
        engine,
        SandboxState {
            limits,
            output: Vec::new(),
            output_bytes: 0,
            config,
        },
    );

    store.limiter(|state| &mut state.limits);

    let max_fuel = store.data().config.max_fuel;
    store.set_fuel(max_fuel)?;

    store.set_epoch_deadline(1);

    Ok(store)
}

pub fn execute_run(
    engine: &Engine,
    instance: &Instance,
    run: &TypedFunc<(), ()>,
    store: &mut Store<SandboxState>,
) -> SandboxResult {
    let (cancel_sender, cancel_receiver) = mpsc::channel::<()>();

    let max_execution_time_seconds = store.data().config.max_execution_time_seconds;

    let timeout_engine = engine.clone();

    let timeout_handle = thread::spawn(move || {
        if cancel_receiver
            .recv_timeout(Duration::from_secs(max_execution_time_seconds))
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
            error: None,
            output: output.clone(),
            execution_time_ms,
            fuel_used,
            memory_used_bytes,
        },

        Err(error) => {
            let execution_error = classify_execution_error(&error);

            SandboxResult {
                success: false,
                message: execution_error.to_string(),
                error: Some(execution_error),
                output,
                execution_time_ms,
                fuel_used,
                memory_used_bytes,
            }
        }
    }
}

pub fn execute_wat(code: &str) -> Result<SandboxResult, SandboxError> {
    execute_wat_with_config(code, SandboxConfig::default())
}

pub fn execute_wat_with_config(
    code: &str,
    config: SandboxConfig,
) -> Result<SandboxResult, SandboxError> {
    let engine =
        create_engine().map_err(|error| SandboxError::EngineCreation(error.to_string()))?;

    let module = Module::new(&engine, code)
        .map_err(|error| SandboxError::InvalidModule(error.to_string()))?;

    let mut store = create_store_with_config(&engine, config)
        .map_err(|error| SandboxError::StoreCreation(error.to_string()))?;

    let instance = instantiate_guest(&engine, &mut store, &module)
        .map_err(|error| SandboxError::Instantiation(error.to_string()))?;

    let run = get_run_function(&instance, &mut store)
        .map_err(|error| SandboxError::InvalidContract(error.to_string()))?;

    Ok(execute_run(&engine, &instance, &run, &mut store))
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
