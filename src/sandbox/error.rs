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

#[derive(Debug)]
pub(super) enum HostError {
    MemoryExportMissing,
    InvalidPointer,
    InvalidTextLength,
    InvalidMemoryRange,
    MemoryAccessOutOfBounds,
    InvalidUtf8,
    OutputLimitExceeded,
}

impl std::fmt::Display for HostError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HostError::MemoryExportMissing => {
                write!(formatter, "guest memory export not found")
            }
            HostError::InvalidPointer => {
                write!(formatter, "guest provided an invalid memory pointer")
            }
            HostError::InvalidTextLength => {
                write!(formatter, "guest provided an invalid text length")
            }
            HostError::InvalidMemoryRange => {
                write!(formatter, "guest provided an invalid memory range")
            }
            HostError::MemoryAccessOutOfBounds => {
                write!(formatter, "guest memory access out of bounds")
            }
            HostError::InvalidUtf8 => {
                write!(formatter, "guest provided invalid UTF-8 text")
            }
            HostError::OutputLimitExceeded => {
                write!(formatter, "guest output limit exceeded")
            }
        }
    }
}

impl std::error::Error for HostError {}

pub(super) fn classify_execution_error(error: &wasmtime::Error) -> ExecutionError {
    if let Some(host_error) = error.downcast_ref::<HostError>() {
        return match host_error {
            HostError::MemoryExportMissing | HostError::MemoryAccessOutOfBounds => {
                ExecutionError::InvalidMemoryAccess
            }
            HostError::InvalidPointer => ExecutionError::InvalidPointer,
            HostError::InvalidTextLength => ExecutionError::InvalidTextLength,
            HostError::InvalidMemoryRange => ExecutionError::InvalidMemoryRange,
            HostError::InvalidUtf8 => ExecutionError::InvalidUtf8,
            HostError::OutputLimitExceeded => ExecutionError::OutputLimitExceeded,
        };
    }

    if let Some(trap) = error.downcast_ref::<wasmtime::Trap>() {
        return match trap {
            wasmtime::Trap::OutOfFuel => ExecutionError::FuelExhausted,
            wasmtime::Trap::Interrupt => ExecutionError::Timeout,
            wasmtime::Trap::MemoryOutOfBounds => ExecutionError::InvalidMemoryAccess,
            _ => ExecutionError::Other(format!("{:#}", error)),
        };
    }

    ExecutionError::Other(format!("{:#}", error))
}
