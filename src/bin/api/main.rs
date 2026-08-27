mod config;
mod handlers;
mod models;
mod state;

use config::load_sandbox_config;

use handlers::{execute, health};

use state::AppState;

use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
};

use std::sync::Arc;
use tokio::sync::Semaphore;

const MAX_REQUEST_BYTES: usize = 1024 * 1024; // 1 MB
const MAX_CONCURRENT_EXECUTIONS: usize = 4;

#[tokio::main]
async fn main() {
    let app = match create_app() {
        Ok(app) => app,

        Err(error) => {
            eprintln!("Failed to start WasmBox API.");
            eprintln!("Reason: {}", error);
            return;
        }
    };

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    println!("WasmBox API running at http://127.0.0.1:3000");

    axum::serve(listener, app).await.unwrap();
}

fn create_app() -> Result<Router, String> {
    let state = AppState {
        execution_semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_EXECUTIONS)),
        sandbox_config: load_sandbox_config()?,
    };

    Ok(Router::new()
        .route("/health", get(health))
        .route("/execute", post(execute))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BYTES))
        .with_state(state))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::models::ExecuteResponse;

    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };

    use tower::ServiceExt;

    #[tokio::test]
    async fn health_endpoint_returns_ok() {
        let app = create_app().expect("App should be created");

        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn execute_valid_guest_returns_ok() {
        let app = create_app().expect("App should be created");

        let body = r#"{
            "code": "(module (func (export \"run\") nop))"
        }"#;

        let request = Request::builder()
            .method("POST")
            .uri("/execute")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn execute_guest_without_run_returns_bad_request() {
        let app = create_app().expect("App should be created");

        let body = r#"{
            "code": "(module (func (export \"hello\")))"
        }"#;

        let request = Request::builder()
            .method("POST")
            .uri("/execute")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn execute_endpoint_returns_guest_output() {
        let app = create_app().expect("App should be created");

        let body = serde_json::json!({
            "code": r#"
                (module
                    (import "host" "print_number"
                        (func $print_number (param i32))
                    )

                    (func (export "run")
                        i32.const 42
                        call $print_number
                    )
                )
            "#
        })
        .to_string();

        let request = Request::builder()
            .method("POST")
            .uri("/execute")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();

        let response: ExecuteResponse = serde_json::from_slice(&body).unwrap();

        assert!(response.success);

        assert_eq!(response.output, vec!["42".to_string()]);
    }

    #[tokio::test]
    async fn infinite_guest_returns_unprocessable_entity() {
        let app = create_app().expect("App should be created");

        let body = serde_json::json!({
            "code": r#"
                (module
                    (func (export "run")
                        (loop $forever
                            br $forever
                        )
                    )
                )
            "#
        })
        .to_string();

        let request = Request::builder()
            .method("POST")
            .uri("/execute")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn oversized_guest_output_returns_unprocessable_entity() {
        let app = create_app().expect("App should be created");

        let body = serde_json::json!({
            "code": r#"
                (module
                    (import "host" "print_text"
                        (func $print_text (param i32 i32))
                    )

                    (memory (export "memory") 2)

                    (func (export "run")
                        i32.const 0
                        i32.const 65537
                        call $print_text
                    )
                )
            "#
        })
        .to_string();

        let request = Request::builder()
            .method("POST")
            .uri("/execute")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn oversized_request_is_rejected() {
        let app = create_app().expect("App should be created");

        let oversized_code = "a".repeat(MAX_REQUEST_BYTES + 1);

        let body = serde_json::json!({
            "code": oversized_code
        })
        .to_string();

        let request = Request::builder()
            .method("POST")
            .uri("/execute")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }
}
