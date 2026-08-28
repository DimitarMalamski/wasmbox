use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use wasmtime::{Config, Engine, Instance, Linker, Module, Store, StoreLimitsBuilder, TypedFunc};

use super::{
    config::{MAX_GUEST_CODE_BYTES, MAX_TABLE_ELEMENTS, SandboxConfig, validate_sandbox_config},
    error::{SandboxError, classify_execution_error},
    host::register_host_functions,
    state::{SandboxResult, SandboxState},
};

struct EpochTimer {
    cancel_sender: mpsc::Sender<()>,
    handle: Option<thread::JoinHandle<()>>,
}

impl EpochTimer {
    fn start(engine: &Engine, seconds: u64) -> Self {
        let (cancel_sender, cancel_receiver) = mpsc::channel::<()>();
        let timer_engine = engine.clone();

        let handle = thread::spawn(move || {
            if cancel_receiver
                .recv_timeout(Duration::from_secs(seconds))
                .is_err()
            {
                timer_engine.increment_epoch();
            }
        });

        Self {
            cancel_sender,
            handle: Some(handle),
        }
    }
}

impl Drop for EpochTimer {
    fn drop(&mut self) {
        let _ = self.cancel_sender.send(());

        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn create_engine() -> wasmtime::Result<Engine> {
    let mut config = Config::new();

    config.consume_fuel(true);
    config.epoch_interruption(true);
    config.wasm_multi_memory(false);
    config.wasm_threads(false);

    Engine::new(&config)
}

fn create_store_with_config(
    engine: &Engine,
    config: SandboxConfig,
) -> wasmtime::Result<Store<SandboxState>> {
    let limits = StoreLimitsBuilder::new()
        .memory_size(config.max_memory_bytes)
        .memories(1)
        .tables(1)
        .instances(1)
        .table_elements(MAX_TABLE_ELEMENTS)
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

fn instantiate_guest(
    engine: &Engine,
    store: &mut Store<SandboxState>,
    module: &Module,
) -> wasmtime::Result<Instance> {
    let mut linker = Linker::new(engine);

    register_host_functions(&mut linker)?;

    linker.instantiate(store, module)
}

fn get_run_function(
    instance: &Instance,
    store: &mut Store<SandboxState>,
) -> wasmtime::Result<TypedFunc<(), ()>> {
    instance
        .get_typed_func::<(), ()>(store, "run")
        .map_err(|_| wasmtime::Error::msg("Guest must export run()."))
}

fn execute_run(
    instance: &Instance,
    run: &TypedFunc<(), ()>,
    store: &mut Store<SandboxState>,
) -> SandboxResult {
    let start = Instant::now();

    let fuel_before = store.get_fuel().unwrap_or(0);

    let execution_result = run.call(&mut *store, ());

    let execution_time_ms = start.elapsed().as_secs_f64() * 1000.0;

    let remaining_fuel = store.get_fuel().unwrap_or(0);
    let fuel_used = fuel_before.saturating_sub(remaining_fuel);

    let memory_used_bytes = match instance.get_memory(&mut *store, "memory") {
        Some(memory) => memory.data_size(&*store),
        None => 0,
    };

    let output = store.data().output.clone();

    match execution_result {
        Ok(_) => SandboxResult {
            success: true,
            message: "Guest executed successfully.".to_string(),
            error: None,
            output,
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
    if code.len() > MAX_GUEST_CODE_BYTES {
        return Err(SandboxError::SourceTooLarge(format!(
            "Guest source is {} bytes, which exceeds the {} byte limit.",
            code.len(),
            MAX_GUEST_CODE_BYTES
        )));
    }

    let engine =
        create_engine().map_err(|error| SandboxError::EngineCreation(error.to_string()))?;

    let module = Module::new(&engine, code)
        .map_err(|error| SandboxError::InvalidModule(error.to_string()))?;

    execute_module_with_config(&engine, &module, config)
}

pub fn execute_file(path: &str) -> Result<SandboxResult, SandboxError> {
    execute_file_with_config(path, SandboxConfig::default())
}

pub fn execute_file_with_config(
    path: &str,
    config: SandboxConfig,
) -> Result<SandboxResult, SandboxError> {
    let engine =
        create_engine().map_err(|error| SandboxError::EngineCreation(error.to_string()))?;

    let module = Module::from_file(&engine, path)
        .map_err(|error| SandboxError::InvalidModule(error.to_string()))?;

    execute_module_with_config(&engine, &module, config)
}

fn execute_module_with_config(
    engine: &Engine,
    module: &Module,
    config: SandboxConfig,
) -> Result<SandboxResult, SandboxError> {
    let config = validate_sandbox_config(config).map_err(SandboxError::InvalidConfig)?;

    let mut store = create_store_with_config(engine, config)
        .map_err(|error| SandboxError::StoreCreation(error.to_string()))?;

    let time_limit_seconds = store.data().config.max_execution_time_seconds;
    let _timer = EpochTimer::start(engine, time_limit_seconds);

    let instance = instantiate_guest(engine, &mut store, module)
        .map_err(|error| SandboxError::Instantiation(error.to_string()))?;

    let run = get_run_function(&instance, &mut store)
        .map_err(|error| SandboxError::InvalidContract(error.to_string()))?;

    Ok(execute_run(&instance, &run, &mut store))
}
