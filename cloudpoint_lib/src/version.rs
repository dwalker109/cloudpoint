use crate::net;
use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use std::net::ToSocketAddrs;

#[derive(Debug, serde::Deserialize)]
pub struct VersionDirEntry {
    name: String,
    size: usize,
    mod_time: DateTime<Utc>,
}

impl VersionDirEntry {
    pub fn fingerprint(&self) -> anyhow::Result<u64> {
        self.name.parse().context("{self.name} is not named with a valid fingerprint")
    }
}

pub fn get_version_dir_entries(
    host: impl ToSocketAddrs,
    path: &str,
) -> anyhow::Result<Vec<VersionDirEntry>> {
    Ok(serde_json::from_slice(&net::get_body(host, path)?)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    #[test]
    fn fingerprint_fails_on_malformed_name() {
        let e = VersionDirEntry{
            name: "foobar".into(),
            size: 123,
            mod_time: Default::default(),
        };

        assert!(e.fingerprint().is_err());
    }

    #[test]
    fn fingerprint_ok_on_valid_name() {
        let e = VersionDirEntry{
            name: "987654321".into(),
            size: 123,
            mod_time: Default::default(),
        };

        assert_eq!(e.fingerprint().unwrap(), 987654321);
    }

    #[test]
    fn can_get_dir_listing() {
        let path = "/sync/abc123/titles/000400001234ABCD/save/";

        let srv = MockServer::start();
        srv.mock(|when, then| {
            when.method("GET").path(path);
            then.status(200).body(
                r#"[
                    {"name":"12345678","size":0,"mod_time":"2026-03-16T14:26:22.425706984Z"},
                    {"name":"abcde123","size":0,"mod_time":"2026-03-17T12:04:29.799632917Z"}
                ]"#,
            );
        });

        let res = get_version_dir_entries((srv.host(), srv.port()), path);
        assert!(res.is_ok());
    }
}
