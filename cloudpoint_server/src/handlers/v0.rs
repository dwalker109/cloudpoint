use crate::{
    AppError, AppState, HexU128,
    services::{chunk, version},
};
use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use chunktree::{tree::MemLeaf, version::Version};
use cloudpoint_lib::ctr::CtrMeta;
use flate2::read::GzDecoder;
use serde_json::json;
use std::io::Read;
use tracing::warn;
use uuid::Uuid;

pub async fn chunk_head(
    State(state): State<AppState>,
    Path((user_key, _shard, cid)): Path<(Uuid, (), HexU128)>,
) -> Result<impl IntoResponse, AppError> {
    let exists = chunk::exists(&user_key, &cid, &state.db_pool).await?;

    match exists {
        true => Ok(StatusCode::OK),
        false => Ok(StatusCode::NO_CONTENT),
    }
}

pub async fn chunk_get(
    State(state): State<AppState>,
    Path((user_key, _shard, cid)): Path<(Uuid, (), HexU128)>,
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
    Path((user_key, _shard, cid)): Path<(Uuid, (), HexU128)>,
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

    chunk::put(&user_key, &cid, &body, decoded.len() as i64, &state.db_pool).await?;

    Ok(StatusCode::CREATED.into_response())
}

pub async fn version_dir_list(
    State(state): State<AppState>,
    Path((user_key, sync_item)): Path<(Uuid, String)>,
) -> Result<impl IntoResponse, AppError> {
    let result = version::latest(&user_key, &sync_item, &state.db_pool).await;

    match result {
        Ok(Some(version)) => {
            let dir_list = json!({
                "paths": [
                    {
                        "name": version.cid,
                        "mtime": version.created_at.timestamp()
                    }
                ]
            });

            Ok(Json(dir_list).into_response())
        }
        Ok(None) => {
            let dir_list = json!({
                "paths": []
            });

            Ok(Json(dir_list).into_response())
        }
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

    version::put(&user_key, &sync_item, &cid, &body, &state.db_pool).await?;

    Ok(StatusCode::CREATED.into_response())
}
