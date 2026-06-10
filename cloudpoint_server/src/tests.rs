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
    assert_eq!(&body[..], b"CLPT!\n");
}

mod v1 {
    use super::*;
    use chunktree::{
        tree::{MemLeaf, Tree},
        version::{ChunkStrategy::PerFile, Concurrency::Serial, Version},
    };
    use cloudpoint_lib::{ctr::CtrMeta, version::RemoteVersionMeta};
    use sqlx::types::chrono::{DateTime, Utc};
    use uuid::{Uuid, uuid};

    const USER_KEY: Uuid = uuid!("ef957928-5486-4f2d-af34-f61a6b5376cc");
    const CHUNK_CID: u128 = 0x45CDD2E492A8BBFC29CC0124CA8BF34B;
    const CHUNK_GZ_DATA: &[u8] = &[
        0x1F, 0x8B, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0xFF, 0x63, 0x64, 0x60, 0x60, 0x00,
        0x00, 0x79, 0xB8, 0xF8, 0x99, 0x04, 0x00, 0x00, 0x00,
    ];
    const SYNC_ITEM: &str = "000400000007E800.savedata";
    const VERSION_CID: u128 = 0x68A9918350F92B62AB8EF8AB0F2EFEA1;

    #[sqlx::test(fixtures("../fixtures/chunks.sql"))]
    async fn bad_path_segments(db_pool: Pool<sqlx::Postgres>) {
        let expected = [
            Request::head(format!("/api/v1/chunk/bad_uuid/{CHUNK_CID:032x}")).body(Body::empty()),
            Request::head(format!("/api/v1/chunk/{USER_KEY}/bad_cid")).body(Body::empty()),
            Request::get(format!("/api/v1/chunk/bad_uuid/{CHUNK_CID:032x}")).body(Body::empty()),
            Request::get(format!("/api/v1/chunk/{USER_KEY}/bad_cid")).body(Body::empty()),
            Request::put(format!("/api/v1/chunk/bad_uuid/{CHUNK_CID:032x}")).body(Body::empty()),
            Request::put(format!("/api/v1/chunk/{USER_KEY}/bad_cid")).body(Body::empty()),
        ];

        let app = super::app(AppState { db_pool });

        for req in expected {
            let response = app.clone().oneshot(req.unwrap()).await.unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
    }

    #[sqlx::test(fixtures("../fixtures/chunks.sql"))]
    async fn chunk_head_ok(db_pool: Pool<sqlx::Postgres>) {
        let app = super::app(AppState { db_pool });

        let response = app
            .oneshot(
                Request::head(format!("/api/v1/chunk/{USER_KEY}/{CHUNK_CID:032x}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[sqlx::test(fixtures("../fixtures/chunks.sql"))]
    async fn chunk_get_ok(db_pool: Pool<sqlx::Postgres>) {
        let app = super::app(AppState { db_pool });

        let response = app
            .oneshot(
                Request::get(format!("/api/v1/chunk/{USER_KEY}/{CHUNK_CID:032x}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], CHUNK_GZ_DATA);
    }

    #[sqlx::test(fixtures("../fixtures/chunks.sql"))]
    async fn chunk_get_no_access_not_found(db_pool: Pool<sqlx::Postgres>) {
        let app = super::app(AppState { db_pool });

        let response = app
            .oneshot(
                Request::get(format!("/api/v1/chunk/{}/{CHUNK_CID:032x}", Uuid::new_v4()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(body.len(), 0);
    }

    #[sqlx::test] // No fixture, intentionally
    async fn chunk_put_ok(db_pool: Pool<sqlx::Postgres>) {
        let app = super::app(AppState { db_pool });

        let response = app
            .clone()
            .oneshot(
                Request::put(format!("/api/v1/chunk/{USER_KEY}/{CHUNK_CID:032x}"))
                    .body(Body::from(CHUNK_GZ_DATA))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        let response = app
            .oneshot(
                Request::get(format!("/api/v1/chunk/{USER_KEY}/{CHUNK_CID:032x}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], CHUNK_GZ_DATA);
    }

    #[sqlx::test] // No fixture, intentionally
    async fn chunk_put_malformed_errors(db_pool: Pool<sqlx::Postgres>) {
        let app = super::app(AppState { db_pool });

        let response = app
            .clone()
            .oneshot(
                Request::put(format!("/api/v1/chunk/{USER_KEY}/{CHUNK_CID:032x}"))
                    .body(Body::from("not gzip data"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let response = app
            .clone()
            .oneshot(
                Request::put(format!("/api/v1/chunk/{USER_KEY}/{:032x}", 012345_u128))
                    .body(Body::from(CHUNK_GZ_DATA))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(fixtures("../fixtures/versions.sql"))]
    async fn version_get_latest_ok(db_pool: Pool<sqlx::Postgres>) {
        let app = super::app(AppState { db_pool });

        let response = app
            .oneshot(
                Request::get(format!("/api/v1/ver/{USER_KEY}/{SYNC_ITEM}/latest"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = &response.into_body().collect().await.unwrap().to_bytes();
        let ver: RemoteVersionMeta = serde_json::from_slice(&body[..]).unwrap();
        assert_eq!(u128::from_str_radix(&ver.cid, 16).unwrap(), VERSION_CID);
        assert_eq!(
            ver.created_at,
            "2026-06-10 17:26:45.526818 +00:00"
                .parse::<DateTime<Utc>>()
                .unwrap()
        );
    }

    #[sqlx::test] // No fixture, intentionally
    async fn version_get_latest_no_data(db_pool: Pool<sqlx::Postgres>) {
        let app = super::app(AppState { db_pool });

        let response = app
            .oneshot(
                Request::get(format!("/api/v1/ver/{USER_KEY}/{SYNC_ITEM}/latest"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        let body = &response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(body.len(), 0);
    }

    #[sqlx::test(fixtures("../fixtures/versions.sql"))]
    async fn version_get_ok(db_pool: Pool<sqlx::Postgres>) {
        let app = super::app(AppState { db_pool });

        let response = app
            .oneshot(
                Request::get(format!(
                    "/api/v1/ver/{USER_KEY}/{SYNC_ITEM}/{VERSION_CID:032x}"
                ))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let ver: Version<MemLeaf, CtrMeta> = postcard::from_bytes(&body[..]).unwrap();
        assert_eq!(ver.fingerprint(), VERSION_CID);
    }

    #[sqlx::test(fixtures("../fixtures/versions.sql"))]
    async fn version_get_no_access_not_found(db_pool: Pool<sqlx::Postgres>) {
        let app = super::app(AppState { db_pool });

        let response = app
            .oneshot(
                Request::get(format!(
                    "/api/v1/ver/{}/{SYNC_ITEM}/{VERSION_CID:032x}",
                    Uuid::new_v4()
                ))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(body.len(), 0);
    }

    #[sqlx::test] // No fixture, intentionally
    async fn ver_put_ok(db_pool: Pool<sqlx::Postgres>) {
        let app = super::app(AppState { db_pool });

        let ver = Version::new(
            &Tree::<MemLeaf>::new(vec![], ()),
            CtrMeta::new(0),
            PerFile,
            Serial,
        )
        .unwrap();

        let response = app
            .clone()
            .oneshot(
                Request::put(format!(
                    "/api/v1/ver/{USER_KEY}/{SYNC_ITEM}/{:032x}",
                    ver.fingerprint()
                ))
                .body(Body::from(postcard::to_allocvec(&ver).unwrap()))
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        let response = app
            .oneshot(
                Request::get(format!(
                    "/api/v1/ver/{USER_KEY}/{SYNC_ITEM}/{:032x}",
                    ver.fingerprint()
                ))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let server_ver: Version<MemLeaf, CtrMeta> = postcard::from_bytes(&body).unwrap();
        assert_eq!(server_ver.fingerprint(), ver.fingerprint());
    }

    #[sqlx::test] // No fixture, intentionally
    async fn ver_put_malformed_errors(db_pool: Pool<sqlx::Postgres>) {
        let app = super::app(AppState { db_pool });

        let response = app
            .clone()
            .oneshot(
                Request::put(format!(
                    "/api/v1/ver/{USER_KEY}/{SYNC_ITEM}/{VERSION_CID:032x}",
                ))
                .body(Body::from("not version data"))
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let ver = Version::new(
            &Tree::<MemLeaf>::new(vec![], ()),
            CtrMeta::new(0),
            PerFile,
            Serial,
        )
        .unwrap();

        let response = app
            .clone()
            .oneshot(
                Request::put(format!(
                    "/api/v1/ver/{USER_KEY}/{SYNC_ITEM}/{:032x}",
                    ver.fingerprint() + 1 // CID no longer matches content
                ))
                .body(Body::from(postcard::to_allocvec(&ver).unwrap()))
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
