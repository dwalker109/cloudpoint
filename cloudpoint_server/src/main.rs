use axum::{
    Router,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, head, put},
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::Duration;
use tower_http::trace::TraceLayer;

mod v1;

#[derive(Clone)]
struct AppState {
    db_pool: PgPool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let state = AppState {
        db_pool: PgPoolOptions::new()
            .max_connections(20)
            .acquire_timeout(Duration::from_secs(5))
            .connect("postgres://postgres:example@localhost:5432/cloudpoint")
            .await?,
    };

    sqlx::migrate!().run(&state.db_pool).await?;

    let v1_router = Router::new()
        .route("/chunk/{u}/{cid}", head(v1::chunk_head))
        .route("/chunk/{u}/{cid}", get(v1::chunk_get))
        .route("/chunk/{u}/{cid}", put(v1::chunk_put))
        .route("/ver/{u}/{si}/{cid}", put(v1::version_put))
        .route("/ver/{u}/{si}/{cid}", get(v1::version_get))
        .route("/ver/{u}/{si}/latest", get(v1::version_meta_latest));

    let app = Router::new()
        .route("/", get(|| async { "CLPT!\n" }))
        .nest("/api/v1", v1_router)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// Make our own error that wraps `anyhow::Error`.
struct AppError(anyhow::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, self.0.to_string()).into_response()
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
