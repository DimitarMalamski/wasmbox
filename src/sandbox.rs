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
pub const MAX_OUTPUT_BYTES: usize = 64 * 1024; // 64 KB

pub const MAX_ALLOWED_FUEL: u64 = 10_000_000;
pub const MAX_ALLOWED_MEMORY_BYTES: usize = 256 * 1024 * 1024; // 256 MB
pub const MAX_ALLOWED_EXECUTION_TIME_SECONDS: u64 = 30;
pub const MAX_ALLOWED_OUTPUT_BYTES: usize = 1024 * 1024; // 1 MB

#[derive(Debug, Clone)]
pub struct SandboxConfig {
    pub max_fuel: u64,
    pub max_memory_bytes: usize,
    pub max_execution_time_seconds: u64,
    pub max_output_bytes: usize,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            max_fuel: MAX_FUEL,
            max_memory_bytes: MAX_MEMORY_BYTES,
            max_execution_time_seconds: MAX_EXECUTION_TIME_SECONDS,
            max_output_bytes: MAX_OUTPUT_BYTES,
        }
    }
}

pub struct SandboxState {
    pub limits: StoreLimits,
    pub output: Vec<String>,
    pub output_bytes: usize,
    pub config: SandboxConfig,
}
pub struct SandboxResult {
    pub success: bool,
    pub message: String,
    pub error: Option<ExecutionError>,
    pub output: Vec<String>,
    pub execution_time_ms: f64,
    pub fuel_used: u64,
    pub memory_used_bytes: usize,
}

#[derive(Debug)]
pub enum SandboxError {
    EngineCreation(String),
    InvalidModule(String),
    StoreCreation(String),
    Instantiation(String),
    InvalidContract(String),
}

#[derive(Debug, Clone)]
pub enum ExecutionError {
    FuelExhausted,
    Timeout,
    InvalidMemoryAccess,
    InvalidPointer,
    InvalidTextLength,
    InvalidMemoryRange,
    InvalidUtf8,
    OutputLimitExceeded,
    Other(String),
}

impl std::fmt::Display for ExecutionError {
    fn fmt(
        &self,
        formatter: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            ExecutionError::FuelExhausted => {
                write!(formatter, "Execution limit exceeded.")
            }

            ExecutionError::Timeout => {
                write!(formatter, "Maximum execution time exceeded.")
            }

            ExecutionError::InvalidMemoryAccess => {
                write!(formatter, "Guest attempted invalid memory access.")
            }

            ExecutionError::InvalidPointer => {
                write!(formatter, "Guest provided an invalid memory pointer.")
            }

            ExecutionError::InvalidTextLength => {
                write!(formatter, "Guest provided an invalid text length.")
            }

            ExecutionError::InvalidMemoryRange => {
                write!(formatter, "Guest provided an invalid memory range.")
            }

            ExecutionError::InvalidUtf8 => {
                write!(formatter, "Guest provided invalid UTF-8 text.")
            }

            ExecutionError::OutputLimitExceeded => {
                write!(formatter, "Guest output limit exceeded.")
            }

            ExecutionError::Other(message) => {
                write!(formatter, "{}", message)
            }
        }
    }
}

impl std::fmt::Display for SandboxError {
    fn fmt(
        &self,
        formatter: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            SandboxError::EngineCreation(message) => {
                write!(formatter, "Failed to create sandbox: {}", message)
            }

            SandboxError::InvalidModule(message) => {
                write!(formatter, "Invalid WebAssembly: {}", message)
            }

            SandboxError::StoreCreation(message) => {
                write!(
                    formatter,
                    "Failed to create sandbox store: {}",
                    message
                )
            }

            SandboxError::Instantiation(message) => {
                write!(
                    formatter,
                    "Could not instantiate guest: {}",
                    message
                )
            }

            SandboxError::InvalidContract(message) => {
                write!(formatter, "{}", message)
            }
        }
    }
}

impl std::error::Error for SandboxError {}

pub fn create_engine() -> wasmtime::Result<Engine> {
    let mut config = Config::new();

    config.consume_fuel(true);
    config.epoch_interruption(true);

    Engine::new(&config)
}

pub fn create_store(
    engine: &Engine,
) -> wasmtime::Result<Store<SandboxState>> {
    create_store_with_config(
        engine,
        SandboxConfig::default(),
    )
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

pub fn classify_execution_error(
    error: &wasmtime::Error,
) -> ExecutionError {
    let error_message = format!("{:#}", error);

    if error_message.contains("fuel") {
        ExecutionError::FuelExhausted
    } else if error_message.contains("wasm trap: interrupt") {
        ExecutionError::Timeout
    } else if error_message.contains("memory access out of bounds") {
        ExecutionError::InvalidMemoryAccess
    } else if error_message.contains("invalid memory pointer") {
        ExecutionError::InvalidPointer
    } else if error_message.contains("invalid text length") {
        ExecutionError::InvalidTextLength
    } else if error_message.contains("invalid memory range") {
        ExecutionError::InvalidMemoryRange
    } else if error_message.contains("invalid UTF-8") {
        ExecutionError::InvalidUtf8
    } else if error_message.contains("output limit exceeded") {
        ExecutionError::OutputLimitExceeded
    } else {
        ExecutionError::Other(error_message)
    }
}

pub fn execute_run(
    engine: &Engine,
    instance: &Instance,
    run: &TypedFunc<(), ()>,
    store: &mut Store<SandboxState>,
) -> SandboxResult {
    let (cancel_sender, cancel_receiver) = mpsc::channel::<()>();

    let max_execution_time_seconds =
        store.data().config.max_execution_time_seconds;

    let timeout_engine = engine.clone();

    let timeout_handle = thread::spawn(move || {
        if cancel_receiver
            .recv_timeout(Duration::from_secs(
                max_execution_time_seconds
            ))
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

pub fn execute_wat(
    code: &str,
) -> Result<SandboxResult, SandboxError> {
    execute_wat_with_config(
        code,
        SandboxConfig::default(),
    )
}

pub fn execute_wat_with_config(
    code: &str,
    config: SandboxConfig,
) -> Result<SandboxResult, SandboxError> {
    let engine = create_engine().map_err(|error| {
        SandboxError::EngineCreation(error.to_string())
    })?;

    let module = Module::new(&engine, code).map_err(|error| {
        SandboxError::InvalidModule(error.to_string())
    })?;

    let mut store =
        create_store_with_config(&engine, config)
            .map_err(|error| {
                SandboxError::StoreCreation(
                    error.to_string()
                )
            })?;

    let instance = instantiate_guest(
        &engine,
        &mut store,
        &module,
    )
    .map_err(|error| {
        SandboxError::Instantiation(error.to_string())
    })?;

    let run = get_run_function(
        &instance,
        &mut store,
    )
    .map_err(|error| {
        SandboxError::InvalidContract(error.to_string())
    })?;

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
        |mut caller: Caller<'_, SandboxState>, number: i32| -> wasmtime::Result<()> {
            let text = number.to_string();
            let text_bytes = text.len();

            let state = caller.data_mut();

            if state
                .output_bytes
                .saturating_add(text_bytes)
                > state.config.max_output_bytes
            {
                return Err(wasmtime::Error::msg(
                    "output limit exceeded",
                ));
            }

            state.output_bytes += text_bytes;
            state.output.push(text);

            Ok(())
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

    let text_bytes = text.len();

    let state = caller.data_mut();

    if state
        .output_bytes
        .saturating_add(text_bytes)
        > state.config.max_output_bytes
    {
        return Err(wasmtime::Error::msg(
            "output limit exceeded",
        ));
    }

    state.output_bytes += text_bytes;
    state.output.push(text);

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

pub fn validate_sandbox_config(
    config: SandboxConfig,
) -> Result<SandboxConfig, String> {
    if config.max_fuel == 0 {
        return Err(
            "WASMBOX_MAX_FUEL must be greater than 0."
                .to_string(),
        );
    }

    if config.max_memory_bytes == 0 {
        return Err(
            "WASMBOX_MAX_MEMORY_BYTES must be greater than 0."
                .to_string(),
        );
    }

    if config.max_execution_time_seconds == 0 {
        return Err(
            "WASMBOX_MAX_EXECUTION_TIME_SECONDS must be greater than 0."
                .to_string(),
        );
    }

    if config.max_output_bytes == 0 {
        return Err(
            "WASMBOX_MAX_OUTPUT_BYTES must be greater than 0."
                .to_string(),
        );
    }

    if config.max_fuel > MAX_ALLOWED_FUEL {
        return Err(format!(
            "WASMBOX_MAX_FUEL cannot exceed {}.",
            MAX_ALLOWED_FUEL
        ));
    }

    if config.max_memory_bytes > MAX_ALLOWED_MEMORY_BYTES {
        return Err(format!(
            "WASMBOX_MAX_MEMORY_BYTES cannot exceed {}.",
            MAX_ALLOWED_MEMORY_BYTES
        ));
    }

    if config.max_execution_time_seconds
        > MAX_ALLOWED_EXECUTION_TIME_SECONDS
    {
        return Err(format!(
            "WASMBOX_MAX_EXECUTION_TIME_SECONDS cannot exceed {}.",
            MAX_ALLOWED_EXECUTION_TIME_SECONDS
        ));
    }

    if config.max_output_bytes > MAX_ALLOWED_OUTPUT_BYTES {
        return Err(format!(
            "WASMBOX_MAX_OUTPUT_BYTES cannot exceed {}.",
            MAX_ALLOWED_OUTPUT_BYTES
        ));
    }

    Ok(config)
}