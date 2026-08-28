#[derive(Debug)]
pub enum SandboxError {
    EngineCreation(String),
    SourceTooLarge(String),
    InvalidModule(String),
    InvalidConfig(String),
    StoreCreation(String),
    Instantiation(String),
    InvalidContract(String),
}

impl std::fmt::Display for SandboxError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SandboxError::EngineCreation(message) => {
                write!(formatter, "Failed to create sandbox: {}", message)
            }

            SandboxError::SourceTooLarge(message) => {
                write!(formatter, "{}", message)
            }

            SandboxError::InvalidModule(message) => {
                write!(formatter, "Invalid WebAssembly: {}", message)
            }

            SandboxError::InvalidConfig(message) => {
                write!(formatter, "{}", message)
            }

            SandboxError::StoreCreation(message) => {
                write!(formatter, "Failed to create sandbox store: {}", message)
            }

            SandboxError::Instantiation(message) => {
                write!(formatter, "Could not instantiate guest: {}", message)
            }

            SandboxError::InvalidContract(message) => {
                write!(formatter, "{}", message)
            }
        }
    }
}

impl std::error::Error for SandboxError {}

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
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

pub(super) fn classify_execution_error(error: &wasmtime::Error) -> ExecutionError {
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
