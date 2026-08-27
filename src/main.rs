use wasmbox::sandbox::{MAX_FUEL, MAX_MEMORY_BYTES, SandboxResult, execute_file};

use std::env;
use std::path::Path;
fn main() {
    let guest_path = match get_guest_path() {
        Some(path) => path,
        None => return,
    };

    if !Path::new(&guest_path).exists() {
        println!("Guest rejected!");
        println!("Reason: File does not exist.");
        return;
    }

    println!("Loading guest: {}", guest_path);
    println!("Starting guest...");

    match execute_file(&guest_path) {
        Ok(result) => {
            print_execution_result(&result);
        }

        Err(error) => {
            println!("Guest rejected!");
            println!("Reason: {}", error);
        }
    }
}

fn print_execution_result(result: &SandboxResult) {
    for line in &result.output {
        println!("Guest says: {}", line);
    }

    println!("Execution time: {:.2} ms", result.execution_time_ms);

    println!("Fuel used: {} / {}", result.fuel_used, MAX_FUEL);

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
