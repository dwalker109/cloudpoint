use crate::HexU128;
use sqlx::{Error, PgPool};
use uuid::Uuid;

pub async fn exists(user_key: &Uuid, cid: &HexU128, db_pool: &PgPool) -> Result<bool, Error> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM chunks WHERE user_key = $1 AND xxhash3_128 = $2) LIMIT 1",
    )
    .bind(user_key)
    .bind(cid.to_bytea())
    .fetch_one(db_pool)
    .await
}

pub async fn get(
    user_key: &Uuid,
    cid: &HexU128,
    db_pool: &PgPool,
) -> Result<Option<Vec<u8>>, Error> {
    let res = sqlx::query_scalar::<_, Vec<u8>>(
        "SELECT body_gz FROM chunks WHERE user_key = $1 AND xxhash3_128 = $2 LIMIT 1",
    )
    .bind(user_key)
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
    cid: &HexU128,
    body: &[u8],
    len: i64,
    db_pool: &PgPool,
) -> Result<(), Error> {
    sqlx::query(
        "INSERT INTO chunks (user_key, xxhash3_128, body_gz, body_len) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
    )
    .bind(user_key)
    .bind(cid.to_bytea())
    .bind(body)
    .bind(len)
    .execute(db_pool)
    .await?;

    Ok(())
}
