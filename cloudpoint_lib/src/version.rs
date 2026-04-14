use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use chunktree::{tree::Leaf, version::Version};
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::{http::CurlHttpClient, sync::CtrArchiveKind};

#[derive(Debug, serde::Deserialize)]
pub struct VersionDirList(Vec<VersionDirEntry>);

impl VersionDirList {
    pub fn try_get(
        client: &CurlHttpClient,
        base_url: &str,
        user_key: &str,
        title_id: u64,
        mode: CtrArchiveKind,
    ) -> Result<VersionDirList> {
        let url = format!("{base_url}/sync/{user_key}/titles/{title_id:016X}/{mode}/");

        let res = client.get(&url, &[("Accept", "application/json")])?;

        match res.status {
            200 => Ok(serde_json::from_slice(&res.body)?),
            404 => Ok(VersionDirList(Vec::with_capacity(0))),
            _ => Err(anyhow!(
                "version dir lookup failed fatally, HTTP {}",
                res.status,
            )),
        }
    }

    pub fn latest(&self) -> Option<&VersionDirEntry> {
        self.0
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
    pub fn fingerprint(&self) -> Result<u64> {
        Ok(u64::from_str_radix(&self.name, 16)?)
    }

    pub fn get_version<T: Leaf>(
        client: &CurlHttpClient,
        base_url: &str,
        user_key: &str,
        title_id: u64,
        mode: CtrArchiveKind,
        fingerprint: u64,
    ) -> Result<Version<T, CtrMeta>> {
        let url =
            format!("{base_url}/sync/{user_key}/titles/{title_id:016X}/{mode}/{fingerprint:016X}",);

        let res = client.get(&url, &[])?;

        match res.status {
            200 => Ok(postcard::from_bytes(&res.body)?),
            _ => Err(anyhow!("version file download failed, HTTP {}", res.status,)),
        }
    }

    pub fn put_version<T: Leaf>(
        client: &CurlHttpClient,
        base_url: &str,
        user_key: &str,
        title_id: u64,
        mode: CtrArchiveKind,
        version: &Version<T, CtrMeta>,
    ) -> Result<()> {
        let url = format!(
            "{base_url}/sync/{user_key}/titles/{title_id:016X}/{mode}/{fingerprint:016X}",
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

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum CtrMeta {
    Unavailable,
    NotInitialized {
        title_version: u16,
    },
    Initialized {
        title_version: u16,
        total_size: u32,
        num_directories: u32,
        num_files: u32,
        duplicate_data: bool,
    },
}

impl CtrMeta {
    pub fn title_version(&self) -> Option<u16> {
        match self {
            CtrMeta::NotInitialized { title_version }
            | CtrMeta::Initialized { title_version, .. } => Some(*title_version),
            _ => None,
        }
    }

    pub fn format_options(&self) -> Option<(u32, u32, u32, bool)> {
        match self {
            CtrMeta::Initialized {
                total_size,
                num_directories,
                num_files,
                duplicate_data,
                ..
            } => Some((*total_size, *num_directories, *num_files, *duplicate_data)),
            _ => None,
        }
    }

    pub fn get_smdh(
        client: &CurlHttpClient,
        base_url: &str,
        user_key: &str,
        title_id: u64,
        mode: CtrArchiveKind,
    ) -> Result<[u8; 0x36c0]> {
        let url = format!("{base_url}/sync/{user_key}/titles/{title_id:016X}/{mode}/smdh",);

        let res = client.get(&url, &[])?;

        match res.status {
            200 => Ok(res.body.try_into().map_err(|_| {
                anyhow!("smdh download not sized correctly, expected 0x36c0 bytes")
            })?),
            _ => Err(anyhow!("smdh download failed, HTTP {}", &res.status,)),
        }
    }

    pub fn put_smdh(
        client: &CurlHttpClient,
        base_url: &str,
        user_key: &str,
        title_id: u64,
        mode: CtrArchiveKind,
        smdh: &[u8; 0x36c0],
    ) -> Result<()> {
        let url = format!("{base_url}/sync/{user_key}/titles/{title_id:016X}/{mode}/smdh",);

        let res = client.put(&url, smdh, &[])?;

        match res.status {
            201 => Ok(()),
            _ => Err(anyhow!("smdh upload failed fatally, HTTP {}", res.status,)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashMap};

    use super::*;
    use chunktree::tree::{MemLeaf, Tree};
    use httpmock::prelude::*;
    use serde::Serialize;

    const USER_KEY: &str = "test_user_key";
    const TITLE_ID: u64 = 0x000400001234ABCD;

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
            when.method("GET")
                .path(format!("/sync/{USER_KEY}/titles/{TITLE_ID:016X}/savedata/"));
            then.status(200).body(
                r#"[
                    {"name":"12345678","size":123,"mtime":123456789},
                    {"name":"abcde123","size":456,"mtime":345678912}
                ]"#,
            );
        });

        let client = CurlHttpClient::new().unwrap();
        let res = VersionDirList::try_get(
            &client,
            &srv.base_url(),
            USER_KEY,
            TITLE_ID,
            CtrArchiveKind::Savedata,
        );

        dbg!(&res);

        assert!(res.is_ok());
        assert_eq!(res.unwrap().0.len(), 2);
    }

    #[test]
    fn can_get_empty_dir_listing_for_200() {
        let srv = MockServer::start();
        srv.mock(|_, then| {
            then.status(200).body("[]");
        });

        let client = CurlHttpClient::new().unwrap();
        let res = VersionDirList::try_get(
            &client,
            &srv.base_url(),
            USER_KEY,
            TITLE_ID,
            CtrArchiveKind::Savedata,
        );

        assert!(res.is_ok());
        assert_eq!(res.unwrap().0.len(), 0);
    }

    #[test]
    fn can_get_empty_dir_listing_for_404() {
        let srv = MockServer::start();
        srv.mock(|_, then| {
            then.status(404);
        });

        let client = CurlHttpClient::new().unwrap();
        let res = VersionDirList::try_get(
            &client,
            &srv.base_url(),
            USER_KEY,
            TITLE_ID,
            CtrArchiveKind::Savedata,
        );

        assert!(res.is_ok());
        assert_eq!(res.unwrap().0.len(), 0);
    }

    #[test]
    fn can_get_version() {
        //TODO! Get a fixture for this instead of duck typing something
        #[derive(Serialize)]
        struct DuckVersion {
            payload: BTreeSet<()>,
            meta: HashMap<String, ()>,
        }

        let srv = MockServer::start();
        srv.mock(|when, then| {
            when.method("GET").path(format!(
                "/sync/{USER_KEY}/titles/{TITLE_ID:016X}/savedata/{fingerprint:016X}",
                fingerprint = 12345678
            ));
            then.status(200).body(
                postcard::to_allocvec(&DuckVersion {
                    payload: BTreeSet::default(),
                    meta: HashMap::default(),
                })
                .unwrap(),
            );
        });

        let client = CurlHttpClient::new().unwrap();
        let res = VersionDirEntry::get_version::<MemLeaf>(
            &client,
            &srv.base_url(),
            USER_KEY,
            TITLE_ID,
            CtrArchiveKind::Savedata,
            12345678,
        );

        assert!(res.is_ok());
    }

    #[test]
    fn version_get_fails_on_malformed_data() {
        let srv = MockServer::start();
        srv.mock(|when, then| {
            when.method("GET").path(format!(
                "/sync/{USER_KEY}/titles/{TITLE_ID:016X}/save/{fingerprint:016X}",
                fingerprint = 12345678
            ));
            then.status(200)
                .body(postcard::to_allocvec(b"junk bytes").unwrap());
        });

        let client = CurlHttpClient::new().unwrap();
        let res = VersionDirEntry::get_version::<MemLeaf>(
            &client,
            &srv.base_url(),
            USER_KEY,
            TITLE_ID,
            CtrArchiveKind::Savedata,
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
        let res = VersionDirEntry::get_version::<MemLeaf>(
            &client,
            &srv.base_url(),
            USER_KEY,
            TITLE_ID,
            CtrArchiveKind::Savedata,
            12345678,
        );

        assert!(res.is_err());
    }

    #[test]
    fn version_put_succeeds_on_new_file() {
        let v = Version::<MemLeaf, CtrMeta>::new(
            &Tree::new(Vec::default(), ()),
            CtrMeta::Initialized {
                title_version: 0x01,
                total_size: 128,
                num_directories: 2,
                num_files: 2,
                duplicate_data: true,
            },
            128,
            512,
            1024,
        )
        .unwrap();

        let srv = MockServer::start();
        srv.mock(|when, then| {
            when.method("PUT").path(format!(
                "/sync/{USER_KEY}/titles/{TITLE_ID:016X}/savedata/{fingerprint:016X}",
                fingerprint = v.fingerprint()
            ));
            then.status(201);
        });

        let client = CurlHttpClient::new().unwrap();
        let res = VersionDirEntry::put_version(
            &client,
            &srv.base_url(),
            USER_KEY,
            TITLE_ID,
            CtrArchiveKind::Savedata,
            &v,
        );

        assert!(res.is_ok());
    }

    #[test]
    fn can_get_smdh() {
        let srv = MockServer::start();
        srv.mock(|when, then| {
            when.method("GET").path(format!(
                "/sync/{USER_KEY}/titles/{TITLE_ID:016X}/extdata/smdh",
            ));
            then.status(200).body(vec![255u8; 0x36c0]);
        });

        let client = CurlHttpClient::new().unwrap();
        let res = CtrMeta::get_smdh(
            &client,
            &srv.base_url(),
            USER_KEY,
            TITLE_ID,
            CtrArchiveKind::Extdata,
        );

        assert!(res.is_ok());
        assert_eq!(res.unwrap().len(), 0x36c0);
    }

    #[test]
    fn can_put_smdh() {
        let smdh = vec![255u8; 0x36c0];

        let srv = MockServer::start();
        srv.mock(|when, then| {
            when.method("PUT").path(format!(
                "/sync/{USER_KEY}/titles/{TITLE_ID:016X}/extdata/smdh",
            ));
            then.status(201);
        });

        let client = CurlHttpClient::new().unwrap();
        let res = CtrMeta::put_smdh(
            &client,
            &srv.base_url(),
            USER_KEY,
            TITLE_ID,
            CtrArchiveKind::Extdata,
            &smdh.try_into().unwrap(),
        );
        dbg!(&res);
        assert!(res.is_ok());
    }
}
