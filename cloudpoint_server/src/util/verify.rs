use std::collections::HashSet;

use anyhow::Result;
use chunktree::{tree::MemLeaf, version::Version};
use cloudpoint_lib::ctr::CtrMeta;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn run(db_pool: &PgPool) -> Result<Vec<(Uuid, Vec<String>)>> {
    let user_keys: Vec<Uuid> = sqlx::query_scalar("SELECT DISTINCT user_key FROM chunks")
        .fetch_all(db_pool)
        .await?;

    let mut report = Vec::new();

    for user_key in user_keys {
        let mut cids = HashSet::new();

        let version_bodies: Vec<Vec<u8>> =
            sqlx::query_scalar("SELECT body FROM versions WHERE user_key = $1")
                .bind(user_key)
                .fetch_all(db_pool)
                .await?;

        for blob in &version_bodies {
            let v: Version<MemLeaf, CtrMeta> = postcard::from_bytes(blob)?;
            let c = v.unique_chunk_hashes();
            cids.extend(c);
        }

        let bytea: Vec<_> = cids.iter().map(|h| h.to_be_bytes().to_vec()).collect();

        let missing: Vec<Vec<u8>> = sqlx::query_scalar(
            "SELECT u.h FROM unnest($2::bytea[]) AS u(h)
             WHERE NOT EXISTS (
                 SELECT 1 FROM chunks c
                 WHERE c.user_key = $1 AND c.xxhash3_128 = u.h
             )",
        )
        .bind(user_key)
        .bind(&bytea)
        .fetch_all(db_pool)
        .await?;

        if !missing.is_empty() {
            let hashes: Vec<String> = missing
                .iter()
                .map(|b| {
                    let arr: [u8; 16] = b.as_slice().try_into().expect("xxhash3_128 is 16 bytes");
                    format!("{:032x}", u128::from_be_bytes(arr))
                })
                .collect();

            tracing::warn!(%user_key, count = hashes.len(), ?hashes, "missing chunks");

            report.push((user_key, hashes));
        }
    }

    Ok(report)
}

#[cfg(test)]
pub mod test {
    use crate::{
        hex_u128::HexU128,
        services::{chunk, version},
    };
    use chunktree::{
        tree::{MemLeaf, Tree},
        version::{ChunkStrategy::PerFile, Concurrency::Serial, Version},
    };
    use cloudpoint_lib::ctr::CtrMeta;
    use uuid::Uuid;

    #[sqlx::test]
    async fn can_verify_with_no_missing_chunks(pg_pool: sqlx::PgPool) {
        let user_1 = Uuid::new_v4();

        let l_a = MemLeaf::new_with_data("a", b"AAAAA");
        let l_b = MemLeaf::new_with_data("b", b"BBBBB");

        let tree_1 = Tree::new([("a".into(), l_a), ("b".into(), l_b)].into_iter(), ());

        let version_1 =
            Version::<MemLeaf, CtrMeta>::new(&tree_1, CtrMeta::new(0), PerFile, Serial).unwrap();

        version::put(
            &user_1,
            "game1",
            &HexU128(version_1.fingerprint()),
            &postcard::to_allocvec(&version_1).unwrap(),
            &pg_pool,
        )
        .await
        .unwrap();

        for n in version_1.unique_chunk_hashes() {
            let c = HexU128(n);
            chunk::put(&user_1, &c, &[], 0, &pg_pool).await.unwrap();
        }

        let report = super::run(&pg_pool).await.unwrap();

        assert_eq!(report.len(), 0);
    }

    #[sqlx::test]
    async fn can_detect_missing_chunks(pg_pool: sqlx::PgPool) {
        let user_1 = Uuid::new_v4();
        let user_2 = Uuid::new_v4();

        let l_a = MemLeaf::new_with_data("a", b"AAAAA");
        let l_b = MemLeaf::new_with_data("b", b"BBBBB");
        let l_c = MemLeaf::new_with_data("c", b"CCCCC");

        let tree_1 = Tree::new(
            [
                ("a".into(), l_a.clone()),
                ("b".into(), l_b.clone()),
                ("c".into(), l_c.clone()),
            ]
            .into_iter(),
            (),
        );

        let tree_2 = Tree::new([("a".into(), l_a), ("b".into(), l_b)].into_iter(), ());

        let version_1 =
            Version::<MemLeaf, CtrMeta>::new(&tree_1, CtrMeta::new(0), PerFile, Serial).unwrap();
        let version_2 =
            Version::<MemLeaf, CtrMeta>::new(&tree_2, CtrMeta::new(0), PerFile, Serial).unwrap();

        version::put(
            &user_1,
            "game1",
            &HexU128(version_1.fingerprint()),
            &postcard::to_allocvec(&version_1).unwrap(),
            &pg_pool,
        )
        .await
        .unwrap();

        version::put(
            &user_2,
            "game1",
            &HexU128(version_2.fingerprint()),
            &postcard::to_allocvec(&version_2).unwrap(),
            &pg_pool,
        )
        .await
        .unwrap();

        for n in version_1.unique_chunk_hashes() {
            let c = HexU128(n);
            chunk::put(&user_1, &c, &[], 0, &pg_pool).await.unwrap();
        }

        let mut version_2_hashes = version_2.unique_chunk_hashes();
        let missing_hash = version_2_hashes.pop_first().unwrap();

        for &n in &version_2_hashes {
            let c = HexU128(n);
            chunk::put(&user_2, &c, &[], 0, &pg_pool).await.unwrap();
        }

        let res = super::run(&pg_pool).await.unwrap();

        let (user, hashes) = &res.first().unwrap();
        assert_eq!(user, &user_2);
        assert_eq!(hashes.len(), 1);
        assert_eq!(hashes[0], format!("{:032x}", missing_hash));
    }
}
