use wasmbox::sandbox::{
    create_engine,
    create_store,
    execute_run,
    get_run_function,
    instantiate_guest,
};

use wasmtime::Module;

use axum::{
    routing::{get, post},
    Json, Router,
};

use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() {
    let app = Router::new()
      .route("/health", get(health))
      .route("/execute", post(execute));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    println!("WasmBox API running at http://127.0.0.1:3000");

    axum::serve(listener, app)
        .await
        .unwrap();
}

async fn health() -> &'static str {
    "WasmBox is running"
}

#[derive(Deserialize)]
struct ExecuteRequest {
    code: String,
}

#[derive(Serialize)]
struct ExecuteResponse {
    success: bool,
    message: String,
    execution_time_ms: Option<f64>,
    fuel_used: Option<u64>,
    memory_used_bytes: Option<usize>,
}

async fn execute(
    Json(request): Json<ExecuteRequest>,
) -> Json<ExecuteResponse> {
    let engine = match create_engine() {
      Ok(engine) => engine,

      Err(error) => {
          return Json(ExecuteResponse {
              success: false,
              message: format!("Failed to create sandbox: {}", error),
              execution_time_ms: None,
              fuel_used: None,
              memory_used_bytes: None,
          });
      }
    };

    let module = match Module::new(&engine, &request.code) {
      Ok(module) => module,
      Err(error) => {
          return Json(ExecuteResponse {
              success: false,
              message: format!("Invalid WebAssembly: {}", error),
              execution_time_ms: None,
              fuel_used: None,
              memory_used_bytes: None,
          });
      }
    };

    let mut store = match create_store(&engine) {
      Ok(store) => store,

      Err(error) => {
          return Json(ExecuteResponse {
              success: false,
              message: format!("Failed to create sandbox store: {}", error),
              execution_time_ms: None,
              fuel_used: None,
              memory_used_bytes: None,
          });
      }
    };

    let instance = match instantiate_guest(
        &engine,
        &mut store,
        &module,
    ) {
        Ok(instance) => instance,

        Err(error) => {
            return Json(ExecuteResponse {
                success: false,
                message: format!("Could not instantiate guest: {}", error),
                execution_time_ms: None,
                fuel_used: None,
                memory_used_bytes: None,
            });
        }
    };

    let run = match get_run_function(&instance, &mut store) {
        Ok(run) => run,

        Err(error) => {
            return Json(ExecuteResponse {
                success: false,
                message: error.to_string(),
                execution_time_ms: None,
                fuel_used: None,
                memory_used_bytes: None,
            });
        }
    };

    let result = execute_run(
        &engine,
        &instance,
        &run,
        &mut store,
    );

    Json(ExecuteResponse {
        success: result.success,
        message: result.message,
        execution_time_ms: Some(result.execution_time_ms),
        fuel_used: Some(result.fuel_used),
        memory_used_bytes: Some(result.memory_used_bytes),
    })
}