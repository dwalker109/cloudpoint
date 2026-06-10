use crate::{AppError, AppState, HexU128};
use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use chunktree::{tree::MemLeaf, version::Version};
use cloudpoint_lib::{ctr::CtrMeta, version::RemoteVersionMeta};
use flate2::read::GzDecoder;
use sqlx::Row;
use std::io::Read;
use tracing::warn;
use uuid::Uuid;

pub async fn chunk_head(
    State(state): State<AppState>,
    Path((user_key, cid)): Path<(Uuid, HexU128)>,
) -> Result<impl IntoResponse, AppError> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM chunks WHERE user_key = $1 AND xxhash3_128 = $2)",
    )
    .bind(&user_key)
    .bind(cid.to_bytea())
    .fetch_one(&state.db_pool)
    .await?;

    match exists {
        true => Ok(StatusCode::OK),
        false => Ok(StatusCode::NO_CONTENT),
    }
}

pub async fn chunk_get(
    State(state): State<AppState>,
    Path((user_key, cid)): Path<(Uuid, HexU128)>,
) -> Result<impl IntoResponse, AppError> {
    let res = sqlx::query_scalar::<_, Vec<u8>>(
        "SELECT body_gz FROM chunks WHERE user_key = $1 AND xxhash3_128 = $2",
    )
    .bind(&user_key)
    .bind(cid.to_bytea())
    .fetch_one(&state.db_pool)
    .await;

    match res {
        Ok(body) => {
            Ok(([(header::CONTENT_TYPE, "application/octet-stream")], body).into_response())
        }
        Err(sqlx::Error::RowNotFound) => Ok(StatusCode::NOT_FOUND.into_response()),
        Err(e) => Err(e.into()),
    }
}

pub async fn chunk_put(
    State(state): State<AppState>,
    Path((user_key, cid)): Path<(Uuid, HexU128)>,
    body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    let mut decoder = GzDecoder::new(body.as_ref());
    let mut decoded = Vec::with_capacity(body.len() * 2);
    if let Err(e) = decoder.read_to_end(&mut decoded) {
        let message = format!("cannot decode uploaded data: {e}");
        warn!(message);

        return Ok((StatusCode::BAD_REQUEST, message).into_response());
    };

    let derived_hash = twox_hash::XxHash3_128::oneshot(&decoded);

    if cid != derived_hash {
        let message = "content id invalid for uploaded data";
        warn!(
            expected = format!("{:032x}", cid.0),
            derived = format!("{derived_hash:032x}"),
            message
        );

        return Ok((StatusCode::BAD_REQUEST, message).into_response());
    }

    sqlx::query(
        "INSERT INTO chunks (user_key, xxhash3_128, body_gz, body_len) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
    )
    .bind(&user_key)
    .bind(cid.to_bytea())
    .bind(body.as_ref())
    .bind(decoded.len() as i64)
    .execute(&state.db_pool)
    .await?;

    Ok(StatusCode::CREATED.into_response())
}

pub async fn version_meta_latest(
    State(state): State<AppState>,
    Path((user_key, sync_item)): Path<(Uuid, String)>,
) -> Result<impl IntoResponse, AppError> {
    let result = sqlx::query(
        "SELECT xxhash3_128, created_at FROM versions WHERE user_key = $1 AND sync_item = $2 ORDER BY created_at DESC",
    )
        .bind(&user_key)
        .bind(&sync_item)
        .fetch_one(&state.db_pool)
        .await;

    match result {
        Ok(row) => Ok(Json(RemoteVersionMeta {
            cid: format!("{:032x}", u128::from_be_bytes(row.try_get("xxhash3_128")?)),
            created_at: row.try_get("created_at")?,
        })
        .into_response()),
        Err(sqlx::Error::RowNotFound) => Ok(StatusCode::NO_CONTENT.into_response()),
        Err(e) => Err(e.into()),
    }
}

pub async fn version_get(
    State(state): State<AppState>,
    Path((user_key, sync_item, cid)): Path<(Uuid, String, HexU128)>,
) -> Result<impl IntoResponse, AppError> {
    let res = sqlx::query_scalar::<_, Vec<u8>>(
        "SELECT body FROM versions WHERE user_key = $1 AND sync_item = $2 AND xxhash3_128 = $3",
    )
    .bind(&user_key)
    .bind(&sync_item)
    .bind(cid.to_bytea())
    .fetch_one(&state.db_pool)
    .await;

    match res {
        Ok(body) => {
            Ok(([(header::CONTENT_TYPE, "application/octet-stream")], body).into_response())
        }
        Err(sqlx::Error::RowNotFound) => Ok(StatusCode::NOT_FOUND.into_response()),
        Err(e) => Err(e.into()),
    }
}

pub async fn version_put(
    State(state): State<AppState>,
    Path((user_key, sync_item, cid)): Path<(Uuid, String, HexU128)>,
    body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    let derived_hash = match postcard::from_bytes::<Version<MemLeaf, CtrMeta>>(&body) {
        Ok(v) => v.fingerprint(),
        Err(e) => {
            let message = format!("cannot decode uploaded data: {e}");
            warn!(message);

            return Ok((StatusCode::BAD_REQUEST, message).into_response());
        }
    };

    if cid != derived_hash {
        let message = "content id invalid for uploaded data";
        warn!(
            expected = format!("{:032x}", cid.0),
            derived = format!("{derived_hash:032x}"),
            message
        );

        return Ok((StatusCode::BAD_REQUEST, message).into_response());
    }

    sqlx::query(
        "INSERT INTO versions (user_key, sync_item, xxhash3_128, body) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
    )
    .bind(&user_key)
    .bind(&sync_item)
    .bind(cid.to_bytea())
    .bind(body.as_ref())
    .execute(&state.db_pool)
    .await?;

    Ok(StatusCode::CREATED.into_response())
}
