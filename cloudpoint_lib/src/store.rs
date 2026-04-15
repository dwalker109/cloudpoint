use crate::http::CurlHttpClient;
use anyhow::anyhow;
use chunktree::store::{StoreError, StoreRead, StoreWrite};
use std::{
    io::{self, Cursor, Read},
    rc::Rc,
};

pub struct HttpStore(Rc<CurlHttpClient>, String);

impl HttpStore {
    pub fn new(client: Rc<CurlHttpClient>, base_url: String) -> Self {
        Self(client, base_url)
    }
}

impl StoreRead for HttpStore {
    fn get_chunk(&self, hash: u64) -> Result<impl Read, StoreError> {
        let res = self
            .0
            .get(&format!("{}/chunks/{}", self.1, hash), &[])
            .map_err(|err| {
                io::Error::new(
                    io::ErrorKind::Other,
                    anyhow!("failed to get chunk {:?}, {}", hash, err),
                )
            })?;

        Ok(Cursor::new(res.body))
    }
}

impl StoreWrite for HttpStore {
    fn put_chunk(&mut self, hash: u64, data: &mut (impl Read + ?Sized)) -> Result<(), StoreError> {
        let url = format!("{}/chunks/{}", self.1, hash);

        let should_upload = self
            .0
            .head(&url, &[])
            .map_err(|err| {
                io::Error::new(
                    io::ErrorKind::Other,
                    anyhow!("failed to check existence of chunk {:?}, {}", hash, err),
                )
            })?
            .status
            != 200;

        let mut body = Vec::new();
        data.read_to_end(&mut body)?;

        if should_upload {
            self.0
                .put(&format!("{}/chunks/{}", self.1, hash), &body, &[])
                .map_err(|err| {
                    io::Error::new(
                        io::ErrorKind::Other,
                        anyhow!("failed to put chunk {:?}, {}", hash, err),
                    )
                })?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::http::CurlHttpClient;
    use chunktree::store::{StoreRead, StoreWrite};
    use httpmock::prelude::*;
    use std::{
        io::{Cursor, Read},
        rc::Rc,
    };

    #[test]
    fn can_put_chunk() {
        let srv = MockServer::start();
        let head_mock = srv.mock(|when, then| {
            when.method("HEAD");
            then.status(404);
        });
        let put_mock = srv.mock(|when, then| {
            when.method("PUT");
            then.status(200);
        });

        let client = CurlHttpClient::new().unwrap();
        let mut store = super::HttpStore::new(Rc::new(client), srv.base_url());

        let hash = 123;
        let data = b"test data";
        store.put_chunk(hash, &mut Cursor::new(data)).unwrap();

        head_mock.assert();
        put_mock.assert();
    }

    #[test]
    fn can_get_chunk() {
        let srv = MockServer::start();
        let get_mock = srv.mock(|when, then| {
            when.method("GET");
            then.status(200).body(b"test data");
        });

        let client = CurlHttpClient::new().unwrap();
        let mut store = super::HttpStore::new(Rc::new(client), srv.base_url());

        let mut buf = Vec::new();
        store
            .get_chunk(0x00)
            .unwrap()
            .read_to_end(&mut buf)
            .unwrap();

        get_mock.assert();
        assert_eq!(buf, b"test data");
    }

    #[test]
    fn skips_upload_when_chunk_exists() {
        let srv = MockServer::start();
        let head_mock = srv.mock(|when, then| {
            when.method("HEAD");
            then.status(200);
        });
        let put_mock = srv.mock(|when, _| {
            when.method("PUT");
        });

        let client = CurlHttpClient::new().unwrap();
        let mut store = super::HttpStore::new(Rc::new(client), srv.base_url());

        let hash = 123;
        let data = b"test data";
        store.put_chunk(hash, &mut Cursor::new(data)).unwrap();

        head_mock.assert();
        put_mock.assert_calls(0);
    }
}
