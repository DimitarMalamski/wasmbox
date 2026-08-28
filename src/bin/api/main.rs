mod config;
mod handlers;
mod models;
mod router;
mod state;

#[cfg(test)]
mod tests;

use router::create_app;

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
