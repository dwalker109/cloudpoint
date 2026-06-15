use crate::HexU128;
use chunktree::{tree::MemLeaf, version::Version};
use cloudpoint_lib::{ctr::CtrMeta, version::RemoteVersionMeta};
use sqlx::{Error, PgPool, Row};
use tracing::warn;
use uuid::Uuid;

pub async fn latest(
    user_key: &Uuid,
    sync_item: &str,
    db_pool: &PgPool,
) -> Result<Option<RemoteVersionMeta>, Error> {
    let result = sqlx::query(
        "SELECT xxhash3_128, created_at FROM versions WHERE user_key = $1 AND sync_item = $2 ORDER BY created_at DESC LIMIT 1",
    )
        .bind(user_key)
        .bind(sync_item)
        .fetch_one(db_pool)
        .await;

    match result {
        Ok(row) => Ok(Some(RemoteVersionMeta {
            cid: format!("{:032x}", u128::from_be_bytes(row.try_get("xxhash3_128")?)),
            created_at: row.try_get("created_at")?,
        })),
        Err(Error::RowNotFound) => Ok(None),
        Err(e) => Err(e),
    }
}

pub async fn get(
    user_key: &Uuid,
    sync_item: &str,
    cid: &HexU128,
    db_pool: &PgPool,
) -> Result<Option<Vec<u8>>, Error> {
    let res = sqlx::query_scalar::<_, Vec<u8>>(
        "SELECT body FROM versions WHERE user_key = $1 AND sync_item = $2 AND xxhash3_128 = $3 LIMIT 1",
    )
    .bind(user_key)
    .bind(sync_item)
    .bind(cid.to_bytea())
    .fetch_one(db_pool)
    .await;

    match res {
        Ok(body) => Ok(Some(body)),
        Err(Error::RowNotFound) => Ok(None),
        Err(e) => Err(e),
    }
}

pub async fn put(
    user_key: &Uuid,
    sync_item: &str,
    cid: &HexU128,
    body: &[u8],
    db_pool: &PgPool,
) -> Result<(), Error> {
    sqlx::query(
        "INSERT INTO versions (user_key, sync_item, xxhash3_128, body) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
    )
    .bind(user_key)
    .bind(sync_item)
    .bind(cid.to_bytea())
    .bind(body)
    .execute(db_pool)
    .await?;

    Ok(())
}

pub fn validate(cid: &HexU128, body: &[u8]) -> Result<(), String> {
    let derived_hash = &match postcard::from_bytes::<Version<MemLeaf, CtrMeta>>(&body) {
        Ok(v) => v.fingerprint(),
        Err(e) => {
            let message = format!("cannot decode uploaded data: {e}");
            warn!(message);

            return Err(message);
        }
    };

    if cid != derived_hash {
        let message = "content id invalid for uploaded data";
        warn!(
            expected = cid.to_string(),
            derived = format!("{derived_hash:032x}"),
            message
        );

        return Err(message.into());
    }

    Ok(())
}
