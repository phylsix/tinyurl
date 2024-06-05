use anyhow::Result;
use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tokio::net::TcpListener;
use tracing::{error, info, level_filters::LevelFilter};
use tracing_subscriber::{fmt::Layer, layer::SubscriberExt, util::SubscriberInitExt, Layer as _};

const LISTENER_ADDR: &str = "127.0.0.1:9876";
const DB_ADDR: &str = "postgres://postgres:postgres@127.0.0.1:5432/tinyurl";

#[derive(Debug, Deserialize)]
struct ShortenRequest {
    url: String,
}

#[derive(Debug, Serialize)]
struct ShortenResponse {
    url: String,
}

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

    let app = Router::new().route("/", post(shorten)).with_state(state);

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

    async fn shorten(&self, url: &str) -> Result<String> {
        let id = nanoid!(6);

        sqlx::query(
            r#"
            INSERT INTO urls (id, url) VALUES ($1, $2)
            ON CONFLICT (url) DO NOTHING
            RETURNING id
            "#,
        )
        .bind(&id)
        .bind(url)
        .fetch_one(&self.db)
        .await?;

        Ok(id)
    }
}

async fn shorten(
    State(state): State<AppState>,
    Json(data): Json<ShortenRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let id = state.shorten(&data.url).await.map_err(|e| {
        error!("Failed to shorten URL: {:?}", e);
        StatusCode::UNPROCESSABLE_ENTITY
    })?;

    let body = Json(ShortenResponse {
        url: format!("{}/{}", LISTENER_ADDR, id),
    });

    Ok((StatusCode::CREATED, body))
}
