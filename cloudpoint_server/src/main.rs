use axum::{
    Router,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, head, put},
};
use serde::{Deserialize, Deserializer};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::Duration;
use tower_http::trace::TraceLayer;

mod v1;

#[derive(Clone)]
pub struct AppState {
    db_pool: PgPool,
}

struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, self.0.to_string()).into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

struct HexU128(u128);

impl<'de> Deserialize<'de> for HexU128 {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = <&str>::deserialize(d)?;
        u128::from_str_radix(s, 16)
            .map(HexU128)
            .map_err(serde::de::Error::custom)
    }
}

impl PartialEq<u128> for HexU128 {
    fn eq(&self, other: &u128) -> bool {
        self.0 == *other
    }
}

impl HexU128 {
    pub fn to_bytea(self) -> [u8; 16] {
        self.0.to_be_bytes()
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let app_state = AppState {
        db_pool: PgPoolOptions::new()
            .max_connections(20)
            .acquire_timeout(Duration::from_secs(5))
            .connect("postgres://postgres:example@localhost:5432/cloudpoint")
            .await?,
    };

    sqlx::migrate!().run(&app_state.db_pool).await?;

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await?;
    axum::serve(listener, app(app_state)).await?;

    Ok(())
}

pub fn app(app_state: AppState) -> Router {
    let v1_router = Router::new()
        .route("/chunk/{u}/{cid}", head(v1::chunk_head))
        .route("/chunk/{u}/{cid}", get(v1::chunk_get))
        .route("/chunk/{u}/{cid}", put(v1::chunk_put))
        .route("/ver/{u}/{si}/{cid}", put(v1::version_put))
        .route("/ver/{u}/{si}/{cid}", get(v1::version_get))
        .route("/ver/{u}/{si}/latest", get(v1::version_meta_latest));

    Router::new()
        .route("/", get(|| async { "CLPT!\n" }))
        .nest("/api/v1", v1_router)
        .layer(TraceLayer::new_for_http())
        .with_state(app_state)
}

#[cfg(test)]
mod tests;
