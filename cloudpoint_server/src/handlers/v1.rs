use crate::{
    app::{AppError, AppState},
    hex_u128::HexU128,
    services::{chunk, version},
};
use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use uuid::Uuid;

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
