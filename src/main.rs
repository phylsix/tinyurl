use anyhow::Result;
use axum::{routing::get, Router};
use sqlx::PgPool;
use tokio::net::TcpListener;
use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::{fmt::Layer, layer::SubscriberExt, util::SubscriberInitExt, Layer as _};

const LISTENER_ADDR: &str = "127.0.0.1:9876";
const DB_ADDR: &str = "postgres://postgres:postgres@localhost:5432/tinyurl";

#[derive(Debug, Clone)]
struct AppState {
    db: PgPool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let layer = Layer::new().with_filter(LevelFilter::INFO);
    tracing_subscriber::registry().with(layer).init();

    let listener = TcpListener::bind(LISTENER_ADDR).await?;
    info!("Listening on: {}", LISTENER_ADDR);

    let state = AppState::try_new().await?;

    let app = Router::new().route("/", get(handler)).with_state(state);

    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}

impl AppState {
    async fn try_new() -> Result<Self> {
        let db = PgPool::connect(DB_ADDR).await?;
        info!("Connected to database: {}", DB_ADDR);

        // create table if not exists
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS urls (
                id CHAR(6) PRIMARY KEY,
                url TEXT NOT NULL UNIQUE
            )
            "#,
        )
        .execute(&db)
        .await?;

        Ok(Self { db })
    }
}

async fn handler() -> &'static str {
    "Hello, World!"
}
