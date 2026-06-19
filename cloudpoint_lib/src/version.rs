use crate::{http::CurlHttpClient, sync::SyncItem};
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use chunktree::{tree::Leaf, version::Version};
use serde::{Serialize, de::DeserializeOwned};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct RemoteVersionMeta {
    pub cid: String,
    pub created_at: DateTime<Utc>,
}

impl RemoteVersionMeta {
    pub fn fingerprint(&self) -> Result<u128> {
        Ok(u128::from_str_radix(&self.cid, 16)?)
    }

    pub fn latest(
        client: &CurlHttpClient,
        base_url: &str,
        user_key: &Uuid,
        sync_item: SyncItem,
    ) -> Result<Option<Self>> {
        log::debug!("getting version dir listing for user.key {user_key}, sync_item {sync_item}");

        let url = format!(
            "{base_url}/api/v1/version/{user_key}/{}/latest",
            PathBuf::from(sync_item).display()
        );

        let res = client.get(&url, &[])?;

        match res.status {
            200 => Ok(Some(serde_json::from_slice(&res.body)?)),
            204 => Ok(None),
            _ => Err(anyhow!("version latest lookup failed: HTTP {}", res.status,)),
        }
    }
}

pub fn get_version<T: Leaf, K: Serialize + DeserializeOwned>(
    client: &CurlHttpClient,
    base_url: &str,
    user_key: &Uuid,
    sync_item: SyncItem,
    cid: u128,
) -> Result<Version<T, K>> {
    log::debug!("getting version for user.key {user_key}, sync_item {sync_item}, cid {cid}",);

    let url = format!(
        "{base_url}/api/v1/version/{user_key}/{si}/{cid:032x}",
        si = PathBuf::from(sync_item).display(),
    );

    let res = client.get(&url, &[])?;

    match res.status {
        200 => Ok(postcard::from_bytes(&res.body)?),
        _ => Err(anyhow!("version file download failed, HTTP {}", res.status,)),
    }
}

pub fn put_version<T: Leaf, K: Serialize + DeserializeOwned>(
    client: &CurlHttpClient,
    base_url: &str,
    user_key: &Uuid,
    sync_item: SyncItem,
    version: &Version<T, K>,
) -> Result<()> {
    let cid = version.fingerprint();

    log::debug!("putting version for user.key {user_key}, sync_item {sync_item}, cid {cid}",);

    let url = format!(
        "{base_url}/api/v1/version/{user_key}/{}/{cid:032x}",
        PathBuf::from(sync_item).display(),
    );

    let serialised = postcard::to_allocvec(&version)?;
    let res = client.put(&url, &serialised, &[])?;

    match res.status {
        201 => Ok(()),
        _ => Err(anyhow!(
            "version file upload failed fatally, HTTP {}",
            res.status,
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use chunktree::tree::{MemLeaf, Tree};
    use httpmock::prelude::*;
    use serde::Serialize;
    use uuid::uuid;

    const USER_KEY: Uuid = uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8");
    const SYNC_ITEM_ID: u64 = 0x000400001234ABCD;
    const FINGERPRINT: u128 = 0x1234567890abcdef;

    #[test]
    fn fingerprint_fails_on_malformed_name() {
        let e = RemoteVersionMeta {
            cid: "foobar".into(),
            created_at: Default::default(),
        };

        assert!(e.fingerprint().is_err());
    }

    #[test]
    fn fingerprint_ok_on_valid_name() {
        let e = RemoteVersionMeta {
            cid: format!("{:032x}", 0xabff99cc_u128),
            created_at: Default::default(),
        };

        assert_eq!(e.fingerprint().unwrap(), 0xabff99cc);
    }

    #[test]
    fn can_get_version_latest() {
        let srv = MockServer::start();
        srv.mock(|when, then| {
            when.method("GET").path(format!(
                "/api/v1/version/{USER_KEY}/{SYNC_ITEM_ID:016X}.savedata/latest"
            ));
            then.status(200).body(
                r#"
                {
                    "cid": "f68de3859be65ff4dd1b57195e466d97",
                    "created_at": "2026-06-09T00:07:40.207850Z"
                }
                "#,
            );
        });

        let client = CurlHttpClient::new("0.0.0").unwrap();
        let res = RemoteVersionMeta::latest(
            &client,
            &srv.base_url(),
            &USER_KEY,
            SyncItem::Savedata(SYNC_ITEM_ID),
        );

        assert!(res.is_ok());
        assert_eq!(
            res.unwrap().unwrap().cid,
            "f68de3859be65ff4dd1b57195e466d97"
        );
    }

    #[test]
    fn can_get_no_content_response_for_no_version() {
        let srv = MockServer::start();
        srv.mock(|_, then| {
            then.status(204);
        });

        let client = CurlHttpClient::new("0.0.0").unwrap();
        let res = RemoteVersionMeta::latest(
            &client,
            &srv.base_url(),
            &USER_KEY,
            SyncItem::Savedata(SYNC_ITEM_ID),
        );

        assert!(res.is_ok());
        assert!(res.unwrap().is_none());
    }

    #[test]
    fn can_get_version() {
        #[derive(Serialize)]
        struct DuckVersion {
            payload: BTreeSet<()>,
            meta: (),
        }

        let srv = MockServer::start();
        srv.mock(|when, then| {
            when.method("GET").path(format!(
                "/api/v1/version/{USER_KEY}/{SYNC_ITEM_ID:016X}.savedata/{FINGERPRINT:032x}",
            ));

            let version = postcard::to_allocvec(&DuckVersion {
                payload: BTreeSet::default(),
                meta: (),
            })
            .unwrap();
            then.status(200).body(version);
        });

        let client = CurlHttpClient::new("0.0.0").unwrap();
        let res = get_version::<MemLeaf, ()>(
            &client,
            &srv.base_url(),
            &USER_KEY,
            SyncItem::Savedata(SYNC_ITEM_ID),
            FINGERPRINT,
        );

        assert!(res.is_ok());
    }

    #[test]
    fn version_get_fails_on_malformed_data() {
        let srv = MockServer::start();
        srv.mock(|when, then| {
            when.method("GET").path(format!(
                "/api/v1/version/{USER_KEY}/{SYNC_ITEM_ID:016X}.savedata/{FINGERPRINT:032x}",
            ));
            then.status(200)
                .body(postcard::to_allocvec(b"invalid postcard bytes").unwrap());
        });

        let client = CurlHttpClient::new("0.0.0").unwrap();
        let res = get_version::<MemLeaf, ()>(
            &client,
            &srv.base_url(),
            &USER_KEY,
            SyncItem::Savedata(SYNC_ITEM_ID),
            FINGERPRINT,
        );

        assert!(res.is_err());
    }

    #[test]
    fn version_get_fails_on_missing_data() {
        let srv = MockServer::start();
        srv.mock(|when, then| {
            when.method("GET");
            then.status(404);
        });

        let client = CurlHttpClient::new("0.0.0").unwrap();
        let res = get_version::<MemLeaf, ()>(
            &client,
            &srv.base_url(),
            &USER_KEY,
            SyncItem::Savedata(SYNC_ITEM_ID),
            FINGERPRINT,
        );

        assert!(res.is_err());
    }

    #[test]
    fn version_put_succeeds_on_new_file() {
        let v = Version::<MemLeaf, ()>::new(
            &Tree::new(Vec::default(), ()),
            (),
            chunktree::version::ChunkStrategy::FixedSize(256),
            chunktree::version::Concurrency::Serial,
        )
        .unwrap();

        let srv = MockServer::start();
        srv.mock(|when, then| {
            when.method("PUT").path(format!(
                "/api/v1/version/{USER_KEY}/{SYNC_ITEM_ID:016X}.savedata/{fingerprint_from_version:032x}",
                fingerprint_from_version = v.fingerprint()
            ));
            then.status(201);
        });

        let client = CurlHttpClient::new("0.0.0").unwrap();
        let res = put_version(
            &client,
            &srv.base_url(),
            &USER_KEY,
            SyncItem::Savedata(SYNC_ITEM_ID),
            &v,
        );

        assert!(res.is_ok());
    }
}
