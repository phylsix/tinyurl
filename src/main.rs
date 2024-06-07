use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use sqlx::{prelude::FromRow, PgPool};
use thiserror::Error;
use tokio::net::TcpListener;
use tracing::{error, info, level_filters::LevelFilter};
use tracing_subscriber::{fmt::Layer, layer::SubscriberExt, util::SubscriberInitExt, Layer as _};

const LISTENER_ADDR: &str = "127.0.0.1:9876";
const DB_ADDR: &str = "postgres://postgres:postgres@127.0.0.1:5432/tinyurl";
const MAX_RETRIES: u8 = 3;

#[derive(Debug, Error)]
enum TinyUrlError {
    #[error("Too many retries (>{0}) to generate unique URL")]
    TooManyShortenRetries(u8),
    #[error("ID not found: {0}")]
    IdNotFound(String),
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
    #[error("Network I/O error: {0}")]
    NetIoError(#[from] std::io::Error),
}

#[derive(Debug, Deserialize)]
struct ShortenRequest {
    url: String,
}

#[derive(Debug, Serialize)]
struct ShortenResponse {
    url: String,
}

#[derive(Debug, FromRow)]
struct UrlRecord {
    #[sqlx(default)]
    id: String,
    #[sqlx(default)]
    url: String,
}

#[derive(Debug, Clone)]
struct AppState {
    db: PgPool,
}

#[tokio::main]
async fn main() -> Result<(), TinyUrlError> {
    let layer = Layer::new().with_filter(LevelFilter::INFO);
    tracing_subscriber::registry().with(layer).init();

    let listener = TcpListener::bind(LISTENER_ADDR).await?;
    info!("Listening on: {}", LISTENER_ADDR);

    let state = AppState::try_new().await?;

    let app = Router::new()
        .route("/", post(shorten))
        .route("/:id", get(redirect))
        .with_state(state);

    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}

impl AppState {
    async fn try_new() -> Result<Self, TinyUrlError> {
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

    async fn shorten(&self, url: &str) -> Result<String, TinyUrlError> {
        let mut id = self._shorten(url).await;
        let mut retries = 0;

        // retry if the generated id already exists
        while id.is_err() && retries < MAX_RETRIES {
            retries += 1;
            id = self._shorten(url).await;
        }

        id.map_err(|_| TinyUrlError::TooManyShortenRetries(MAX_RETRIES))
    }

    async fn _shorten(&self, url: &str) -> Result<String, TinyUrlError> {
        let id = nanoid!(6);

        let res: UrlRecord = sqlx::query_as(
            r#"
            INSERT INTO urls (id, url) VALUES ($1, $2)
            ON CONFLICT (url) DO UPDATE SET url = EXCLUDED.url
            RETURNING id
            "#,
        )
        .bind(&id)
        .bind(url)
        .fetch_one(&self.db)
        .await?;

        Ok(res.id)
    }

    async fn get_url_by_id(&self, id: &str) -> Result<String, TinyUrlError> {
        let url = sqlx::query_scalar(
            r#"
            SELECT url FROM urls WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await?;

        url.ok_or(TinyUrlError::IdNotFound(id.to_string()))
    }
}

async fn shorten(
    State(state): State<AppState>,
    Json(data): Json<ShortenRequest>,
) -> Result<impl IntoResponse, TinyUrlError> {
    let id = state.shorten(&data.url).await?;

    let body = Json(ShortenResponse {
        url: format!("{}/{}", LISTENER_ADDR, id),
    });

    Ok((StatusCode::CREATED, body))
}

async fn redirect(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, TinyUrlError> {
    let url = state.get_url_by_id(&id).await?;

    let mut headers = http::header::HeaderMap::new();
    headers.insert(header::LOCATION, url.parse().unwrap());

    Ok((StatusCode::PERMANENT_REDIRECT, headers))
}

impl IntoResponse for TinyUrlError {
    fn into_response(self) -> Response {
        error!("{}", self);

        let resp = match &self {
            TinyUrlError::TooManyShortenRetries(_) => {
                (StatusCode::UNPROCESSABLE_ENTITY, "URL generation failed")
            }
            TinyUrlError::IdNotFound(_) => (StatusCode::NOT_FOUND, "Resource Not Found"),
            TinyUrlError::DatabaseError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")
            }
            TinyUrlError::NetIoError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")
            }
        };

        resp.into_response()
    }
}
