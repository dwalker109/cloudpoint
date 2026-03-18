use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use itertools::Itertools;

#[derive(Debug, serde::Deserialize)]
pub struct VersionDirList(Vec<VersionDirEntry>);

impl VersionDirList {
    pub fn try_get(base_url: &str, user_key: &str, title_id: u64) -> Result<VersionDirList> {
        let url = format!("{base_url}/sync/{user_key}/titles/{title_id}/save/");

        let res = minreq::get(url)
            .with_header("Accept", "application/json")
            .send()?;

        match res.status_code {
            200 => Ok(serde_json::from_slice(&res.as_bytes())?),
            404 => Ok(VersionDirList(Vec::with_capacity(0))),
            _ => Err(anyhow!(
                "version dir lookup failed, HTTP {}",
                res.status_code
            )),
        }
    }

    pub fn latest(&self) -> Option<&VersionDirEntry> {
        self.0
            .as_slice()
            .iter()
            .sorted_by(|a, b| a.mod_time.cmp(&b.mod_time))
            .last()
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct VersionDirEntry {
    name: String,
    size: usize,
    mod_time: DateTime<Utc>,
}

impl VersionDirEntry {
    pub fn fingerprint(&self) -> Result<u64> {
        self.name
            .parse()
            .context("{self.name} is not named with a valid fingerprint")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    const USER_KEY: &str = "test_user_key";
    const TITLE_ID: u64 = 0x000400001234ABCD;

    #[test]
    fn fingerprint_fails_on_malformed_name() {
        let e = VersionDirEntry {
            name: "foobar".into(),
            size: 123,
            mod_time: Default::default(),
        };

        assert!(e.fingerprint().is_err());
    }

    #[test]
    fn fingerprint_ok_on_valid_name() {
        let e = VersionDirEntry {
            name: "987654321".into(),
            size: 123,
            mod_time: Default::default(),
        };

        assert_eq!(e.fingerprint().unwrap(), 987654321);
    }

    #[test]
    fn can_get_dir_listing() {
        let srv = MockServer::start();
        srv.mock(|when, then| {
            when.method("GET").path(format!("/sync/{USER_KEY}/titles/{TITLE_ID}/save/"));
            then.status(200).body(
                r#"[
                    {"name":"12345678","size":0,"mod_time":"2026-03-16T14:26:22.425706984Z"},
                    {"name":"abcde123","size":0,"mod_time":"2026-03-17T12:04:29.799632917Z"}
                ]"#,
            );
        });

        let res = VersionDirList::try_get(&srv.base_url(), USER_KEY, TITLE_ID);

        assert!(res.is_ok());
        assert_eq!(res.unwrap().0.len(), 2);
    }

    #[test]
    fn can_get_empty_dir_listing_for_404() {
        let srv = MockServer::start();
        srv.mock(|_, then| {
            then.status(404);
        });

        let res = VersionDirList::try_get(&srv.base_url(), USER_KEY, TITLE_ID);

        assert!(res.is_ok());
        assert_eq!(res.unwrap().0.len(), 0);
    }
}
