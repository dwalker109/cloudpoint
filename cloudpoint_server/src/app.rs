use axum::{
    Router,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, head, put},
};
use sqlx::PgPool;
use tower_http::trace::TraceLayer;

use crate::handlers;

#[derive(Clone)]
pub struct AppState {
    pub(crate) db_pool: PgPool,
}

pub struct AppError(anyhow::Error);

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

pub fn make(app_state: AppState) -> Router {
    let v0_router = Router::new()
        .route("/{u}/chunks/{shard}/{cid}", head(handlers::v0::chunk_head))
        .route("/{u}/chunks/{shard}/{cid}", get(handlers::v0::chunk_get))
        .route("/{u}/chunks/{shard}/{cid}", put(handlers::v0::chunk_put))
        .route("/{u}/archives/{si}/{cid}", put(handlers::v0::version_put))
        .route("/{u}/archives/{si}/{cid}", get(handlers::v0::version_get))
        .route("/{u}/archives/{si}/", get(handlers::v0::version_dir_list));

    let v1_router = Router::new()
        .route("/preflight", get(handlers::v1::preflight_get))
        .route("/chunk/{u}/{cid}", head(handlers::v1::chunk_head))
        .route("/chunk/{u}/{cid}", get(handlers::v1::chunk_get))
        .route("/chunk/{u}/{cid}", put(handlers::v1::chunk_put))
        .route("/ver/{u}/{si}/{cid}", put(handlers::v1::version_put))
        .route("/ver/{u}/{si}/{cid}", get(handlers::v1::version_get))
        .route(
            "/ver/{u}/{si}/latest",
            get(handlers::v1::version_meta_latest),
        );

    Router::new()
        .route("/", get(|| async { "CLPT!" }))
        .route("/version", get(|| async { env!("CARGO_PKG_VERSION") }))
        .nest("/sync", v0_router)
        .nest("/api/v1", v1_router)
        .layer(TraceLayer::new_for_http())
        .with_state(app_state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use sqlx::Pool;
    use tower::ServiceExt;

    #[sqlx::test]
    async fn hello_world(db_pool: Pool<sqlx::Postgres>) {
        let app = super::make(AppState { db_pool });

        let response = app
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"CLPT!");
    }

    mod v0;
    mod v1;
}
