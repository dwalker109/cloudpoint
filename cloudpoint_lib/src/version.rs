use std::path::PathBuf;

use crate::{ctr::CtrArchiveId, http::CurlHttpClient};
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use chunktree::{tree::Leaf, version::Version};
use itertools::Itertools;
use serde::{Serialize, de::DeserializeOwned};
use uuid::Uuid;

#[derive(Debug, serde::Deserialize)]
pub struct VersionDirList {
    paths: Vec<VersionDirEntry>,
}

impl VersionDirList {
    pub fn try_get(
        client: &CurlHttpClient,
        base_url: &str,
        user_key: &Uuid,
        archive_id: CtrArchiveId,
    ) -> Result<VersionDirList> {
        let url = format!(
            "{base_url}/sync/{user_key}/archives/{archive_id}/?json",
            archive_id = PathBuf::from(archive_id).display()
        );

        let res = client.get(&url, &[])?;

        match res.status {
            200 => Ok(serde_json::from_slice(&res.body)?),
            _ => Err(anyhow!(
                "version dir lookup failed fatally, HTTP {}",
                res.status,
            )),
        }
    }

    pub fn latest(&self) -> Option<&VersionDirEntry> {
        self.paths
            .as_slice()
            .iter()
            .sorted_by(|a, b| a.mtime.cmp(&b.mtime))
            .last()
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct VersionDirEntry {
    name: String,
    #[serde(with = "chrono::serde::ts_seconds")]
    mtime: DateTime<Utc>,
}

impl VersionDirEntry {
    pub fn fingerprint(&self) -> Result<u128> {
        Ok(u128::from_str_radix(&self.name, 16)?)
    }

    pub fn get_version<T: Leaf, K: Serialize + DeserializeOwned>(
        client: &CurlHttpClient,
        base_url: &str,
        user_key: &Uuid,
        archive_id: CtrArchiveId,
        fingerprint: u128,
    ) -> Result<Version<T, K>> {
        let url = format!(
            "{base_url}/sync/{user_key}/archives/{archive_id}/{fingerprint:016X}",
            archive_id = PathBuf::from(archive_id).display(),
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
        archive_id: CtrArchiveId,
        version: &Version<T, K>,
    ) -> Result<()> {
        let url = format!(
            "{base_url}/sync/{user_key}/archives/{archive_id}/{fingerprint:016X}",
            archive_id = PathBuf::from(archive_id).display(),
            fingerprint = version.fingerprint(),
        );

        let body = postcard::to_allocvec(&version)?;
        let res = client.put(&url, &body, &[])?;

        match res.status {
            201 => Ok(()),
            _ => Err(anyhow!(
                "version file upload failed fatally, HTTP {}",
                res.status,
            )),
        }
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
    const ARCHIVE_ID: u64 = 0x000400001234ABCD;

    #[test]
    fn fingerprint_fails_on_malformed_name() {
        let e = VersionDirEntry {
            name: "foobar".into(),
            mtime: Default::default(),
        };

        assert!(e.fingerprint().is_err());
    }

    #[test]
    fn fingerprint_ok_on_valid_name() {
        let e = VersionDirEntry {
            name: "000400000007AF00".into(),
            mtime: Default::default(),
        };

        assert_eq!(e.fingerprint().unwrap(), 0x000400000007AF00);
    }

    #[test]
    fn can_get_dir_listing() {
        let srv = MockServer::start();
        srv.mock(|when, then| {
            when.method("GET").path(format!(
                "/sync/{USER_KEY}/archives/{ARCHIVE_ID:016X}.savedata/"
            ));
            then.status(200).body(
                r#"{"paths": [
                    {"name":"12345678","size":123,"mtime":123456789},
                    {"name":"abcde123","size":456,"mtime":345678912}
                ]}"#,
            );
        });

        let client = CurlHttpClient::new().unwrap();
        let res = VersionDirList::try_get(
            &client,
            &srv.base_url(),
            &USER_KEY,
            CtrArchiveId::Savedata(ARCHIVE_ID),
        );

        assert!(res.is_ok());
        assert_eq!(res.unwrap().paths.len(), 2);
    }

    #[test]
    fn can_get_empty_dir_listing_for_200() {
        let srv = MockServer::start();
        srv.mock(|_, then| {
            then.status(200).body(r#"{ "paths": [] }"#);
        });

        let client = CurlHttpClient::new().unwrap();
        let res = VersionDirList::try_get(
            &client,
            &srv.base_url(),
            &USER_KEY,
            CtrArchiveId::Savedata(ARCHIVE_ID),
        );

        assert!(res.is_ok());
        assert_eq!(res.unwrap().paths.len(), 0);
    }

    #[test]
    fn can_get_version() {
        //TODO! Get a fixture for this instead of duck typing something
        #[derive(Serialize)]
        struct DuckVersion {
            payload: BTreeSet<()>,
            meta: (),
        }

        let srv = MockServer::start();
        srv.mock(|when, then| {
            when.method("GET").path(format!(
                "/sync/{USER_KEY}/archives/{ARCHIVE_ID:016X}.savedata/{fingerprint:016X}",
                fingerprint = 12345678
            ));
            then.status(200).body(
                postcard::to_allocvec(&DuckVersion {
                    payload: BTreeSet::default(),
                    meta: (),
                })
                .unwrap(),
            );
        });

        let client = CurlHttpClient::new().unwrap();
        let res = VersionDirEntry::get_version::<MemLeaf, ()>(
            &client,
            &srv.base_url(),
            &USER_KEY,
            CtrArchiveId::Savedata(ARCHIVE_ID),
            12345678,
        );

        assert!(res.is_ok());
    }

    #[test]
    fn version_get_fails_on_malformed_data() {
        let srv = MockServer::start();
        srv.mock(|when, then| {
            when.method("GET").path(format!(
                "/sync/{USER_KEY}/archives/{ARCHIVE_ID:016X}.savedata/{fingerprint:016X}",
                fingerprint = 12345678
            ));
            then.status(200)
                .body(postcard::to_allocvec(b"junk bytes").unwrap());
        });

        let client = CurlHttpClient::new().unwrap();
        let res = VersionDirEntry::get_version::<MemLeaf, ()>(
            &client,
            &srv.base_url(),
            &USER_KEY,
            CtrArchiveId::Savedata(ARCHIVE_ID),
            12345678,
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

        let client = CurlHttpClient::new().unwrap();
        let res = VersionDirEntry::get_version::<MemLeaf, ()>(
            &client,
            &srv.base_url(),
            &USER_KEY,
            CtrArchiveId::Savedata(ARCHIVE_ID),
            12345678,
        );

        assert!(res.is_err());
    }

    #[test]
    fn version_put_succeeds_on_new_file() {
        let v = Version::<MemLeaf, ()>::new(
            &Tree::new(Vec::default(), ()),
            (),
            chunktree::version::ChunkStrategy::Cdc(128, 512, 1024),
            chunktree::version::Concurrency::Serial,
        )
        .unwrap();

        let srv = MockServer::start();
        srv.mock(|when, then| {
            when.method("PUT").path(format!(
                "/sync/{USER_KEY}/archives/{ARCHIVE_ID:016X}.savedata/{fingerprint:016X}",
                fingerprint = v.fingerprint()
            ));
            then.status(201);
        });

        let client = CurlHttpClient::new().unwrap();
        let res = VersionDirEntry::put_version(
            &client,
            &srv.base_url(),
            &USER_KEY,
            CtrArchiveId::Savedata(ARCHIVE_ID),
            &v,
        );

        assert!(res.is_ok());
    }
}
