use std::collections::HashSet;

use anyhow::Result;
use chunktree::{tree::MemLeaf, version::Version};
use cloudpoint_lib::ctr::CtrMeta;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn run(db_pool: &PgPool) -> Result<()> {
    let user_keys: Vec<Uuid> = sqlx::query_scalar("SELECT DISTINCT user_key FROM chunks")
        .fetch_all(db_pool)
        .await?;

    for user_key in user_keys {
        let version_bodies: Vec<Vec<u8>> =
            sqlx::query_scalar("SELECT body FROM versions WHERE user_key = $1")
                .bind(user_key)
                .fetch_all(db_pool)
                .await?;

        let mut cids = HashSet::new();

        for blob in &version_bodies {
            let v: Version<MemLeaf, CtrMeta> = postcard::from_bytes(blob)?;
            let c = v.unique_chunk_hashes();
            cids.extend(c);
        }

        let bytea_cids: Vec<_> = cids.iter().map(|h| h.to_be_bytes().to_vec()).collect();

        let deleted: Vec<Uuid> = sqlx::query_scalar(
            "DELETE FROM chunks c
            WHERE c.user_key = $1
              AND NOT (c.xxhash3_128 = ANY($2::bytea[]))
            RETURNING c.id",
        )
        .bind(user_key)
        .bind(bytea_cids)
        .fetch_all(db_pool)
        .await?;

        if deleted.len() > 0 {
            tracing::info!(%user_key, count = deleted.len(), "GC'd orphan chunks");
        }
    }

    Ok(())
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
    async fn can_gc(pg_pool: sqlx::PgPool) {
        let user_1 = Uuid::new_v4();
        let user_2 = Uuid::new_v4();

        let l_a = MemLeaf::new_with_data("a", b"AAAAA");
        let l_b = MemLeaf::new_with_data("b", b"BBBBB");
        let l_c = MemLeaf::new_with_data("c", b"CCCCC");
        let l_d = MemLeaf::new_with_data("d", b"DDDDD");
        let l_e = MemLeaf::new_with_data("e", b"EEEEE");

        let tree_1 = Tree::new(
            [
                ("a".into(), l_a.clone()),
                ("b".into(), l_b.clone()),
                ("c".into(), l_c.clone()),
            ]
            .into_iter(),
            (),
        );

        let tree_2 = Tree::new(
            [
                ("c".into(), l_c.clone()),
                ("d".into(), l_d.clone()),
                ("e".into(), l_e.clone()),
            ]
            .into_iter(),
            (),
        );

        let tree_3 = Tree::new(
            [
                ("a".into(), l_a),
                ("b".into(), l_b),
                ("c".into(), l_c),
                ("d".into(), l_d),
                ("e".into(), l_e),
            ]
            .into_iter(),
            (),
        );

        let version_1 =
            Version::<MemLeaf, CtrMeta>::new(&tree_1, CtrMeta::new(0), PerFile, Serial).unwrap();
        let version_2 =
            Version::<MemLeaf, CtrMeta>::new(&tree_2, CtrMeta::new(0), PerFile, Serial).unwrap();
        let version_3 =
            Version::<MemLeaf, CtrMeta>::new(&tree_3, CtrMeta::new(0), PerFile, Serial).unwrap();

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
            &user_1,
            "game1",
            &HexU128(version_2.fingerprint()),
            &postcard::to_allocvec(&version_2).unwrap(),
            &pg_pool,
        )
        .await
        .unwrap();

        version::put(
            &user_2,
            "game1",
            &HexU128(version_3.fingerprint()),
            &postcard::to_allocvec(&version_3).unwrap(),
            &pg_pool,
        )
        .await
        .unwrap();

        for n in version_1.unique_chunk_hashes() {
            let c = HexU128(n);
            chunk::put(&user_1, &c, &[], 0, &pg_pool).await.unwrap();
        }

        for n in version_2.unique_chunk_hashes() {
            let c = HexU128(n);
            chunk::put(&user_1, &c, &[], 0, &pg_pool).await.unwrap();
        }

        for n in version_3.unique_chunk_hashes() {
            let c = HexU128(n);
            chunk::put(&user_2, &c, &[], 0, &pg_pool).await.unwrap();
        }

        let total_1 =
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM chunks where user_key = $1")
                .bind(&user_1)
                .fetch_one(&pg_pool)
                .await
                .unwrap();
        let total_2 =
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM chunks where user_key = $1")
                .bind(&user_2)
                .fetch_one(&pg_pool)
                .await
                .unwrap();

        assert_eq!(total_1, 5);
        assert_eq!(total_2, 5);

        let res = sqlx::query("DELETE FROM versions WHERE xxhash3_128 = $1")
            .bind(&HexU128(version_2.fingerprint()).to_bytea())
            .execute(&pg_pool)
            .await
            .unwrap()
            .rows_affected();

        assert_eq!(res, 1);

        super::run(&pg_pool).await.unwrap();

        let total_1 =
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM chunks where user_key = $1")
                .bind(&user_1)
                .fetch_one(&pg_pool)
                .await
                .unwrap();
        let total_2 =
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM chunks where user_key = $1")
                .bind(&user_2)
                .fetch_one(&pg_pool)
                .await
                .unwrap();

        assert_eq!(total_1, 3);
        assert_eq!(total_2, 5);
    }
}
