//! Binary entry point: initialises logging, builds the state and serves the
//! HTTP API until the process is stopped (Ctrl+C or SIGTERM).

use std::sync::Arc;

use rust_pdf_generator::api;
use rust_pdf_generator::config::{AppConfig, ENV_LOG};
use rust_pdf_generator::state::AppState;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let config = AppConfig::from_env();
    let address = config.binding_address();

    tracing::info!(
        %address,
        templates = %config.templates_dir.display(),
        output = %config.output_dir.display(),
        max_concurrent_renders = config.max_concurrent_renders,
        max_payload_bytes = config.max_payload_bytes,
        "starting the pdf generator"
    );

    let state = AppState::new(config)?;
    let app = api::router(Arc::clone(&state));

    let listener = TcpListener::bind(&address)
        .await
        .map_err(|err| format!("could not bind to {address}: {err}"))?;

    tracing::info!(address = %listener.local_addr()?, "listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("server stopped");
    Ok(())
}

/// Configures `tracing` from `RUST_LOG`, defaulting to `info`.
fn init_tracing() {
    let filter = EnvFilter::try_from_env(ENV_LOG)
        .or_else(|_| EnvFilter::try_new("info,rust_pdf_generator=debug"))
        .expect("the default log filter is valid");

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .compact()
        .init();
}

/// Resolves when the process receives Ctrl+C or SIGTERM.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Ctrl+C handler can be installed");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("SIGTERM handler can be installed")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => tracing::info!("received Ctrl+C, shutting down"),
        () = terminate => tracing::info!("received SIGTERM, shutting down"),
    }
}
