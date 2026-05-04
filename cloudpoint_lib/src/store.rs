use crate::http::CurlHttpClient;
use anyhow::anyhow;
use chunktree::store::{StoreError, StoreRead, StoreWrite};
use flate2::{
    Compression,
    read::{GzDecoder, GzEncoder},
};
use std::{
    io::{self, Cursor, Read},
    rc::Rc,
};
use uuid::Uuid;

pub struct HttpStore(Rc<CurlHttpClient>, String, Uuid);

impl HttpStore {
    pub fn new(client: Rc<CurlHttpClient>, base_url: String, user_key: Uuid) -> Self {
        Self(client, base_url, user_key)
    }

    fn fq_hash_url(&self, hash: u128) -> String {
        let [msb, ..] = hash.to_be_bytes();

        format!(
            "{}/sync/{}/chunks/{:02x}/{:032x}",
            self.1, self.2, msb, hash
        )
    }
}

impl StoreRead for HttpStore {
    fn get_chunk(&self, hash: u128) -> Result<impl Read, StoreError> {
        let res = self.0.get(&self.fq_hash_url(hash), &[]).map_err(|err| {
            io::Error::new(
                io::ErrorKind::Other,
                anyhow!("failed to get chunk {:?}, {}", hash, err),
            )
        })?;

        Ok(GzDecoder::new(Cursor::new(res.body)))
    }
}

impl StoreWrite for HttpStore {
    fn put_chunk(&mut self, hash: u128, data: &mut (impl Read + ?Sized)) -> Result<(), StoreError> {
        let url = self.fq_hash_url(hash);

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

        if should_upload {
            let mut body = Vec::new();
            let mut gzip_encoder = GzEncoder::new(data, Compression::best());
            gzip_encoder.read_to_end(&mut body)?;

            self.0.put(&url, &body, &[]).map_err(|err| {
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
    use flate2::{Compression, read::GzEncoder};
    use httpmock::prelude::*;
    use std::{
        io::{Cursor, Read},
        rc::Rc,
    };
    use uuid::Uuid;

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
        let mut store = super::HttpStore::new(Rc::new(client), srv.base_url(), Uuid::new_v4());

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
            then.status(200).body({
                let mut encoder = GzEncoder::new(Cursor::new(b"test data"), Compression::none());
                let mut buf = vec![];
                encoder.read_to_end(&mut buf).ok();

                buf
            });
        });

        let client = CurlHttpClient::new().unwrap();
        let store = super::HttpStore::new(Rc::new(client), srv.base_url(), Uuid::new_v4());

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
        let mut store = super::HttpStore::new(Rc::new(client), srv.base_url(), Uuid::new_v4());

        let hash = 123;
        let data = b"test data";
        store.put_chunk(hash, &mut Cursor::new(data)).unwrap();

        head_mock.assert();
        put_mock.assert_calls(0);
    }
}
