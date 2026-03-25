use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use chunktree::{tree::Leaf, version::Version};
use itertools::Itertools;

use crate::sync::CtrArchiveMode;

#[derive(Debug, serde::Deserialize)]
pub struct VersionDirList(Vec<VersionDirEntry>);

impl VersionDirList {
    pub fn try_get(
        base_url: &str,
        user_key: &str,
        title_id: u64,
        mode: CtrArchiveMode,
    ) -> Result<VersionDirList> {
        let url = format!("{base_url}/sync/{user_key}/titles/{title_id}/{mode}/");

        let res = minreq::get(url)
            .with_header("Accept", "application/json")
            .send()?;

        match res.status_code {
            200 => Ok(serde_json::from_slice(&res.as_bytes())?),
            404 => Ok(VersionDirList(Vec::with_capacity(0))),
            _ => Err(anyhow!(
                "version dir lookup failed fatally, HTTP {}, {}",
                res.status_code,
                res.reason_phrase
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
        self.name
            .parse()
            .context("{self.name} is not named with a valid fingerprint")
    }

    pub fn get_version<T: Leaf>(
        base_url: &str,
        user_key: &str,
        title_id: u64,
        mode: CtrArchiveMode,
        fingerprint: u64,
    ) -> Result<Version<T>> {
        let url = format!("{base_url}/sync/{user_key}/titles/{title_id}/{mode}/{fingerprint}",);

        let res = minreq::get(url).send()?;

        match res.status_code {
            200 => Ok(postcard::from_bytes(res.as_bytes())?),
            _ => Err(anyhow!(
                "version file download failed, HTTP {}, {}",
                res.status_code,
                res.reason_phrase
            )),
        }
    }

    pub fn put_version<T: Leaf>(
        base_url: &str,
        user_key: &str,
        title_id: u64,
        mode: CtrArchiveMode,
        version: &Version<T>,
    ) -> Result<()> {
        let url = format!(
            "{base_url}/sync/{user_key}/titles/{title_id}/{mode}/{}",
            version.fingerprint(),
        );

        let res = minreq::put(url)
            .with_body(postcard::to_allocvec(&version)?)
            .send()?;

        match res.status_code {
            201 => Ok(()),
            _ => Err(anyhow!(
                "version file upload failed fatally, HTTP {}, {}",
                res.status_code,
                res.reason_phrase
            )),
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
            name: "987654321".into(),
            mtime: Default::default(),
        };

        assert_eq!(e.fingerprint().unwrap(), 987654321);
    }

    #[test]
    fn can_get_dir_listing() {
        let srv = MockServer::start();
        srv.mock(|when, then| {
            when.method("GET")
                .path(format!("/sync/{USER_KEY}/titles/{TITLE_ID}/save/"));
            then.status(200).body(
                r#"[
                    {"name":"12345678","size":123,"mtime":"2026-03-16T14:26:22.425706984Z"},
                    {"name":"abcde123","size":456,"mtime":"2026-03-17T12:04:29.799632917Z"}
                ]"#,
            );
        });

        let res = VersionDirList::try_get(
            &srv.base_url(),
            USER_KEY,
            TITLE_ID,
            CtrArchiveMode::Savedata,
        );

        assert!(res.is_ok());
        assert_eq!(res.unwrap().0.len(), 2);
    }

    #[test]
    fn can_get_empty_dir_listing_for_200() {
        let srv = MockServer::start();
        srv.mock(|_, then| {
            then.status(200).body("{}");
        });

        let res = VersionDirList::try_get(
            &srv.base_url(),
            USER_KEY,
            TITLE_ID,
            CtrArchiveMode::Savedata,
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

        let res = VersionDirList::try_get(
            &srv.base_url(),
            USER_KEY,
            TITLE_ID,
            CtrArchiveMode::Savedata,
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
                "/sync/{USER_KEY}/titles/{TITLE_ID}/extdata/12345678"
            ));
            then.status(200).body(
                postcard::to_allocvec(&DuckVersion {
                    payload: BTreeSet::default(),
                    meta: HashMap::default(),
                })
                .unwrap(),
            );
        });

        let res = VersionDirEntry::get_version::<MemLeaf>(
            &srv.base_url(),
            USER_KEY,
            TITLE_ID,
            CtrArchiveMode::Savedata,
            12345678,
        );

        assert!(res.is_ok());
    }

    #[test]
    fn version_get_fails_on_malformed_data() {
        let srv = MockServer::start();
        srv.mock(|when, then| {
            when.method("GET")
                .path(format!("/sync/{USER_KEY}/titles/{TITLE_ID}/save/12345678"));
            then.status(200)
                .body(postcard::to_allocvec(b"junk bytes").unwrap());
        });

        let res = VersionDirEntry::get_version::<MemLeaf>(
            &srv.base_url(),
            USER_KEY,
            TITLE_ID,
            CtrArchiveMode::Savedata,
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

        let res = VersionDirEntry::get_version::<MemLeaf>(
            &srv.base_url(),
            USER_KEY,
            TITLE_ID,
            CtrArchiveMode::Savedata,
            12345678,
        );

        assert!(res.is_err());
    }

    #[test]
    fn version_put_succeeds_on_new_file() {
        let v = Version::<MemLeaf>::new(
            &Tree::new(Vec::default(), ()),
            HashMap::default(),
            128,
            512,
            1024,
        )
        .unwrap();

        let srv = MockServer::start();
        srv.mock(|when, then| {
            when.method("PUT").path(format!(
                "/sync/{USER_KEY}/titles/{TITLE_ID}/save/{}",
                v.fingerprint()
            ));
            then.status(201);
        });

        let res = VersionDirEntry::put_version(
            &srv.base_url(),
            USER_KEY,
            TITLE_ID,
            CtrArchiveMode::Savedata,
            &v,
        );

        assert!(res.is_ok());
    }

    // #[test]
    // fn version_put_fails_on_existing_file() {
    //     let v = Version::<MemLeaf>::new(
    //         &Tree::new(Vec::default(), ()),
    //         HashMap::default(),
    //         128,
    //         512,
    //         1024,
    //     )
    //     .unwrap();

    //     let srv = MockServer::start();
    //     srv.mock(|when, then| {
    //         when.method("PUT");
    //         then.status(403);
    //     });

    //     let res = VersionDirEntry::put_version(&srv.base_url(), USER_KEY, TITLE_ID, &v);

    //     assert!(res.is_err());
    // }
}
