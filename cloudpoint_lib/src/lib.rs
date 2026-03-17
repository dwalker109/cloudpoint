pub mod title;

pub mod net {
    use anyhow::{Result, anyhow};
    use chrono::{DateTime, Utc};
    use nom::{
        IResult, Parser,
        bytes::{tag, take_until},
        combinator::rest,
        sequence::preceded,
    };
    use std::{
        io::{Read, Write},
        net::{TcpStream, ToSocketAddrs},
    };

    #[derive(Debug, serde::Deserialize)]
    struct DirList {
        name: String,
        size: usize,
        mod_time: DateTime<Utc>,
    }

    pub fn get_dir_list(host: impl ToSocketAddrs, path: &str) -> Result<Vec<DirList>> {
        Ok(serde_json::from_slice(&get_body(host, path)?)?)
    }

    fn get_body(host: impl ToSocketAddrs, path: &str) -> Result<Vec<u8>> {
        let mut stream = TcpStream::connect(host)?;

        let req = format!(
            "GET {path} HTTP/1.1\r\n\
            Host: cloudpoint\r\n\
            Accept: application/json\r\n\
            Connection: close\r\n\
            \r\n"
        );

        stream.write_all(req.as_bytes())?;

        let mut buf = Vec::new();
        stream.read_to_end(&mut buf)?;

        let (_, body) = parse_http1(&buf)
            .map_err(|e| anyhow!("failed to parse directory listing for {path}: {e}"))?;

        Ok(body.to_vec())

    }

    fn parse_http1(input: &[u8]) -> IResult<&[u8], &[u8]> {
        preceded(preceded(take_until("\r\n\r\n"), tag("\r\n\r\n")), rest).parse(input)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use httpmock::prelude::*;

        #[test]
        fn can_get_body() {
            let path = "/test";

            let srv = MockServer::start();
            srv.mock(|when, then| {
                when.method("GET").path(path);
                then.status(200);
            });

            let res = get_body((srv.host(), srv.port()), path);
            assert!(res.is_ok());

        }

        #[test]
        fn can_get_dir_listing() {
            let path = "/sync/abc123/titles/000400001234ABCD/save/";

            let srv = MockServer::start();
            srv.mock(|when, then| {
                when.method("GET").path(path);
                then.status(200).body(
                    r#"[
                        {"name":"12345678.cps","size":0,"mod_time":"2026-03-16T14:26:22.425706984Z"},
                        {"name":"abcde123.cps","size":0,"mod_time":"2026-03-17T12:04:29.799632917Z"}
                    ]"#,
                );
            });

            let res = get_dir_list((srv.host(), srv.port()), path);
            assert!(res.is_ok());
        }
    }
}
