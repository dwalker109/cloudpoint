use std::cmp::Ordering;

use crate::{
    app::{AppError, AppState},
    hex_u128::HexU128,
    postcard::Postcard,
    services::{chunk, version},
};
use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::{
        StatusCode,
        header::{self, HeaderMap},
    },
    response::IntoResponse,
};
use cloudpoint_lib::preflight::Update;
use tracing::warn;
use uuid::Uuid;

pub async fn preflight_get(headers: HeaderMap) -> Result<impl IntoResponse, AppError> {
    let Some(ua) = headers.get("User-Agent") else {
        warn!(headers = format!("{headers:?}"), "no User-Agent header");

        return Ok(StatusCode::BAD_REQUEST.into_response());
    };

    let Some((_, ver)) = ua.to_str()?.split_once('/') else {
        warn!(
            headers = format!("{headers:?}"),
            "malformed User-Agent header"
        );

        return Ok(StatusCode::BAD_REQUEST.into_response());
    };

    let Ok(app_ver) = semver::Version::parse(ver) else {
        warn!(ver, "malformed client app_ver in User-Agent header");

        return Ok(StatusCode::BAD_REQUEST.into_response());
    };

    let server_ver = semver::Version::parse(env!("CARGO_PKG_VERSION"))?;

    let res = match app_ver.major.cmp(&server_ver.major) {
        Ordering::Less => Postcard(Update::Client {
            to_version: app_ver,
            url: String::default(),
            required: true,
        })
        .into_response(),

        Ordering::Greater => Postcard(Update::Server {
            to_version: server_ver,
            required: true,
        })
        .into_response(),

        Ordering::Equal => StatusCode::OK.into_response(),
        // TODO! offer optional updates
    };

    Ok(res)
}

pub async fn chunk_head(
    State(state): State<AppState>,
    Path((user_key, cid)): Path<(Uuid, HexU128)>,
) -> Result<impl IntoResponse, AppError> {
    let exists = chunk::exists(&user_key, &cid, &state.db_pool).await?;

    match exists {
        true => Ok(StatusCode::OK),
        false => Ok(StatusCode::NO_CONTENT),
    }
}

pub async fn chunk_get(
    State(state): State<AppState>,
    Path((user_key, cid)): Path<(Uuid, HexU128)>,
) -> Result<impl IntoResponse, AppError> {
    let res = chunk::get(&user_key, &cid, &state.db_pool).await;

    match res {
        Ok(Some(body)) => {
            Ok(([(header::CONTENT_TYPE, "application/octet-stream")], body).into_response())
        }
        Ok(None) => Ok(StatusCode::NOT_FOUND.into_response()),
        Err(e) => Err(e.into()),
    }
}

pub async fn chunk_put(
    State(state): State<AppState>,
    Path((user_key, cid)): Path<(Uuid, HexU128)>,
    body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    let len = match chunk::validate(&cid, &body) {
        Ok(len) => len,
        Err(message) => return Ok((StatusCode::BAD_REQUEST, message).into_response()),
    };

    chunk::put(&user_key, &cid, &body, len, &state.db_pool).await?;

    Ok(StatusCode::CREATED.into_response())
}

pub async fn version_meta_latest(
    State(state): State<AppState>,
    Path((user_key, sync_item)): Path<(Uuid, String)>,
) -> Result<impl IntoResponse, AppError> {
    let result = version::latest(&user_key, &sync_item, &state.db_pool).await;

    match result {
        Ok(Some(version)) => Ok(Json(version).into_response()),
        Ok(None) => Ok(StatusCode::NO_CONTENT.into_response()),
        Err(e) => Err(e.into()),
    }
}

pub async fn version_get(
    State(state): State<AppState>,
    Path((user_key, sync_item, cid)): Path<(Uuid, String, HexU128)>,
) -> Result<impl IntoResponse, AppError> {
    let res = version::get(&user_key, &sync_item, &cid, &state.db_pool).await;

    match res {
        Ok(Some(body)) => {
            Ok(([(header::CONTENT_TYPE, "application/octet-stream")], body).into_response())
        }
        Ok(None) => Ok(StatusCode::NOT_FOUND.into_response()),
        Err(e) => Err(e.into()),
    }
}

pub async fn version_put(
    State(state): State<AppState>,
    Path((user_key, sync_item, cid)): Path<(Uuid, String, HexU128)>,
    body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    if let Err(message) = version::validate(&cid, &body) {
        return Ok((StatusCode::BAD_REQUEST, message).into_response());
    };

    version::put(&user_key, &sync_item, &cid, &body, &state.db_pool).await?;

    Ok(StatusCode::CREATED.into_response())
}
