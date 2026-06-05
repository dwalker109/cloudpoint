use crate::{AppError, AppState};
use anyhow::anyhow;
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
use uuid::Uuid;

pub async fn chunk_head(
    State(state): State<AppState>,
    Path((user_key, cid)): Path<(Uuid, String)>,
) -> Result<impl IntoResponse, AppError> {
    let xxhash3_128 = u128::from_str_radix(&cid, 16)?;

    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM chunks WHERE user_key = $1 AND xxhash3_128 = $2)",
    )
    .bind(&user_key)
    .bind(xxhash3_128.to_be_bytes())
    .fetch_one(&state.db_pool)
    .await?;

    match exists {
        true => Ok(StatusCode::OK),
        false => Ok(StatusCode::NO_CONTENT),
    }
}

pub async fn chunk_get(
    State(state): State<AppState>,
    Path((user_key, cid)): Path<(Uuid, String)>,
) -> Result<impl IntoResponse, AppError> {
    let xxhash3_128 = u128::from_str_radix(&cid, 16)?;

    let body = sqlx::query_scalar::<_, Vec<u8>>(
        "SELECT body FROM chunks WHERE user_key = $1 AND xxhash3_128 = $2",
    )
    .bind(&user_key)
    .bind(xxhash3_128.to_be_bytes())
    .fetch_one(&state.db_pool)
    .await?;

    Ok(([(header::CONTENT_TYPE, "application/octet-stream")], body))
}

pub async fn chunk_put(
    State(state): State<AppState>,
    Path((user_key, cid)): Path<(Uuid, String)>,
    body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    let xxhash3_128 = u128::from_str_radix(&cid, 16)?;

    let mut buf = Vec::new();
    let mut decoder = GzDecoder::new(body.as_ref());
    decoder.read_to_end(&mut buf)?;

    let expected_hash = xxhash3_128;
    let derived_hash = twox_hash::XxHash3_128::oneshot(&buf);

    if expected_hash != derived_hash {
        return Err(anyhow!(
            "provided content id is not valid for the provided data"
        ))?;
    }

    sqlx::query("INSERT INTO chunks (user_key, xxhash3_128, body) VALUES ($1, $2, $3)")
        .bind(&user_key)
        .bind(xxhash3_128.to_be_bytes())
        .bind(body.as_ref())
        .execute(&state.db_pool)
        .await?;

    Ok(StatusCode::CREATED)
}

pub async fn version_put(
    State(state): State<AppState>,
    Path((user_key, sync_item, cid)): Path<(Uuid, String, String)>,
    body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    let xxhash3_128 = u128::from_str_radix(&cid, 16)?;

    let expected_version = xxhash3_128;
    let derived_version = postcard::from_bytes::<Version<MemLeaf, CtrMeta>>(&body)?.fingerprint();

    if expected_version != derived_version {
        return Err(anyhow!(
            "provided content id is not valid for the provided data"
        ))?;
    }

    sqlx::query(
        "INSERT INTO versions (user_key, sync_item, xxhash3_128, body) VALUES ($1, $2, $3, $4)",
    )
    .bind(&user_key)
    .bind(&sync_item)
    .bind(xxhash3_128.to_be_bytes())
    .bind(body.as_ref())
    .execute(&state.db_pool)
    .await?;

    Ok(StatusCode::CREATED)
}

pub async fn version_meta_latest(
    State(state): State<AppState>,
    Path((user_key, sync_item)): Path<(Uuid, String)>,
) -> Result<impl IntoResponse, AppError> {
    let row = sqlx::query(
        "SELECT xxhash3_128, created_at FROM versions WHERE user_key = $1 AND sync_item = $2 ORDER BY created_at DESC",
    )
    .bind(&user_key)
    .bind(&sync_item)
    .fetch_one(&state.db_pool)
    .await?;

    Ok(Json(RemoteVersionMeta {
        cid: format!("{:032x}", u128::from_be_bytes(row.try_get("xxhash3_128")?)),
        created_at: row.try_get("created_at")?,
    }))
}

pub async fn version_file_get(
    State(state): State<AppState>,
    Path((user_key, sync_item, cid)): Path<(Uuid, String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let xxhash3_128 = u128::from_str_radix(&cid, 16)?;

    let body = sqlx::query_scalar::<_, Vec<u8>>(
        "SELECT body FROM versions WHERE user_key = $1 AND sync_item = $2 AND xxhash3_128 = $3",
    )
    .bind(&user_key)
    .bind(&sync_item)
    .bind(xxhash3_128.to_be_bytes())
    .fetch_one(&state.db_pool)
    .await?;

    Ok(([(header::CONTENT_TYPE, "application/octet-stream")], body))
}
