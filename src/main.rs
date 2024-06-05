use anyhow::Result;
use axum::{routing::get, Router};
use tokio::net::TcpListener;
use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::{fmt::Layer, layer::SubscriberExt, util::SubscriberInitExt, Layer as _};

const LISTENER_ADDR: &str = "127.0.0.1:9876";

#[derive(Debug, Clone)]
struct AppState {}

#[tokio::main]
async fn main() -> Result<()> {
    let layer = Layer::new().with_filter(LevelFilter::INFO);
    tracing_subscriber::registry().with(layer).init();

    let listener = TcpListener::bind(LISTENER_ADDR).await?;
    info!("Listening on: {}", LISTENER_ADDR);

    let state = AppState {};

    let app = Router::new()
            .route("/", get(handler))
            .with_state(state);

    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}

async fn handler() -> &'static str {
    "Hello, World!"
}
