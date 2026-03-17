use anyhow::{Result, anyhow};
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
use nom::sequence::separated_pair;

pub fn get_body(host: impl ToSocketAddrs, path: &str) -> Result<Vec<u8>> {
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

    let (_, (_, body)) = parse_http1(&buf)
        .map_err(|e| anyhow!("failed to parse directory listing for {path}: {e}"))?;

    Ok(body.into())

}

fn parse_http1(input: &[u8]) -> IResult<&[u8], (&[u8], &[u8])> {
    let http_separator = "\r\n\r\n";
    separated_pair(take_until(http_separator), tag(http_separator), rest).parse(input)
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
            then.status(200).body("Hello world!");
        });

        let res = get_body((srv.host(), srv.port()), path);

        assert_eq!(res.unwrap(), b"Hello world!");
    }

    #[test]
    fn can_parse_with_body_when_present() {
        let raw = "Headers\r\n\r\nBody";

        let (input, (h, b)) = parse_http1(raw.as_bytes()).unwrap();

        assert!(input.is_empty());
        assert_eq!(h, b"Headers");
        assert_eq!(b, b"Body");
    }

    #[test]
    fn can_parse_with_body_when_missing() {
        let raw = "Headers\r\n\r\n";

        let (input, (h, b)) = parse_http1(raw.as_bytes()).unwrap();

        assert!(input.is_empty());
        assert_eq!(h, b"Headers");
        assert!(b.is_empty());
    }

    #[test]
    fn parsing_fails_on_malformed_http() {
        let raw = "Malformed";

        let parsed = parse_http1(raw.as_bytes());

        assert!(parsed.is_err());
    }
}
