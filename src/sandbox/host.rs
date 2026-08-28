use wasmtime::{Caller, Linker};

use super::error::HostError;
use super::state::SandboxState;

pub(super) fn register_host_functions(linker: &mut Linker<SandboxState>) -> wasmtime::Result<()> {
    register_print_number(linker)?;
    register_print_text(linker)?;

    Ok(())
}

fn register_print_number(linker: &mut Linker<SandboxState>) -> wasmtime::Result<()> {
    linker.func_wrap(
        "host",
        "print_number",
        |mut caller: Caller<'_, SandboxState>, number: i32| -> wasmtime::Result<()> {
            let text = number.to_string();
            let text_bytes = text.len();

            let state = caller.data_mut();

            if state.output_bytes.saturating_add(text_bytes) > state.config.max_output_bytes {
                return Err(HostError::OutputLimitExceeded.into());
            }

            state.output_bytes += text_bytes;
            state.output.push(text);

            Ok(())
        },
    )?;

    Ok(())
}

fn register_print_text(linker: &mut Linker<SandboxState>) -> wasmtime::Result<()> {
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
        .ok_or(HostError::MemoryExportMissing)?;

    let data = memory.data(&caller);

    let start = usize::try_from(pointer).map_err(|_| HostError::InvalidPointer)?;

    let length = usize::try_from(length).map_err(|_| HostError::InvalidTextLength)?;

    let end = start
        .checked_add(length)
        .ok_or(HostError::InvalidMemoryRange)?;

    if end > data.len() {
        return Err(HostError::MemoryAccessOutOfBounds.into());
    }

    let bytes = &data[start..end];

    let text = std::str::from_utf8(bytes)
        .map_err(|_| HostError::InvalidUtf8)?
        .to_string();

    let text_bytes = text.len();

    let state = caller.data_mut();

    if state.output_bytes.saturating_add(text_bytes) > state.config.max_output_bytes {
        return Err(HostError::OutputLimitExceeded.into());
    }

    state.output_bytes += text_bytes;
    state.output.push(text);

    Ok(())
}
