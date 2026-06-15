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
    let app = super::app(AppState { db_pool });

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
