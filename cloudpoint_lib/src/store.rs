use crate::http::CurlHttpClient;
use chunktree::store::{StoreError, StoreRead, StoreWrite};
use flate2::{
    Compression,
    read::{GzDecoder, GzEncoder},
};
use lru::LruCache;
use std::{
    collections::HashSet,
    io::{self, Cursor, Read},
    num::NonZeroUsize,
    rc::Rc,
    sync::RwLock,
};
use uuid::Uuid;

pub struct HttpStore {
    http_client: Rc<CurlHttpClient>,
    base_url: String,
    user_key: Uuid,
    upload_dedupe: HashSet<u128>,
    download_dedupe: RwLock<LruCache<u128, Vec<u8>>>,
}

impl HttpStore {
    pub fn new(client: Rc<CurlHttpClient>, base_url: String, user_key: Uuid) -> Self {
        Self {
            http_client: client,
            base_url,
            user_key,
            upload_dedupe: HashSet::new(),
            download_dedupe: RwLock::new(LruCache::new(NonZeroUsize::new(32).unwrap())),
        }
    }

    fn fq_hash_url(&self, hash: u128) -> String {
        format!(
            "{}/api/v1/chunk/{}/{:032x}",
            self.base_url, self.user_key, hash
        )
    }
}

impl StoreRead for HttpStore {
    fn get_chunk(&self, hash: u128) -> Result<impl Read, StoreError> {
        log::debug!("getting store chunk for hash {hash:032x}");

        let mut lru = self
            .download_dedupe
            .write()
            .expect("should be able to lock store lru");

        if let Some(body) = lru.get(&hash) {
            log::debug!("retrieved chunk {hash:032x} from lru cache");

            return Ok(GzDecoder::new(Cursor::new(body.clone())));
        }

        let res = self
            .http_client
            .get(&self.fq_hash_url(hash), &[])
            .map_err(|err| {
                io::Error::other(format!("transport failure for chunk {hash:032x}, {err}"))
            })?;

        match res.status {
            200 => log::debug!("completed download for chunk {hash:032x}"),
            _ => {
                return Err(io::Error::other(format!(
                    "server rejected get for chunk {hash:032x}, {} (HTTP {})",
                    String::from_utf8_lossy(&res.body),
                    res.status,
                ))
                .into());
            }
        }

        log::debug!("adding chunk {hash:032x} to lru cache");
        lru.put(hash, res.body.clone());

        Ok(GzDecoder::new(Cursor::new(res.body)))
    }
}

impl StoreWrite for HttpStore {
    fn put_chunks<T: chunktree::tree::Leaf>(
        &mut self,
        leaf_chunks: &chunktree::tree::LeafChunks,
        source: &T,
    ) -> Result<(), StoreError> {
        for &(hash, offset, length) in leaf_chunks.chunks() {
            if self.upload_dedupe.insert(hash) {
                self.put_chunk(hash, &mut source.read_chunk(offset, length)?)?;
            } else {
                log::debug!("skipped upload of chunk {hash:032x}, duplicated within session");
            }
        }

        Ok(())
    }

    fn put_chunk(&mut self, hash: u128, data: &mut (impl Read + ?Sized)) -> Result<(), StoreError> {
        log::debug!("putting store chunk for hash {hash:032x}");

        let url = self.fq_hash_url(hash);

        let should_upload = self
            .http_client
            .head(&url, &[])
            .map_err(|err| {
                io::Error::other(format!(
                    "failed to check existence of chunk {hash:032x}, {err}"
                ))
            })?
            .status
            == 204;

        if should_upload {
            log::debug!("uploading for hash {hash:032x}");

            let mut body = Vec::new();
            let mut gzip_encoder = GzEncoder::new(data, Compression::best());
            gzip_encoder.read_to_end(&mut body)?;

            let res = self.http_client.put(&url, &body, &[]).map_err(|err| {
                io::Error::other(format!("transport failure for chunk {hash:032x}, {err}"))
            })?;

            match res.status {
                201 => log::debug!("completed upload for chunk {hash:032x}"),
                _ => {
                    return Err(io::Error::other(format!(
                        "server rejected put for chunk {hash:032x}, {} (HTTP {})",
                        String::from_utf8_lossy(&res.body),
                        res.status,
                    ))
                    .into());
                }
            }
        } else {
            log::debug!("skipped upload for hash {hash:032x}, already exists")
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
            then.status(204);
        });
        let put_mock = srv.mock(|when, then| {
            when.method("PUT");
            then.status(201);
        });

        let client = CurlHttpClient::new("0.0.0").unwrap();
        let mut store = super::HttpStore::new(Rc::new(client), srv.base_url(), Uuid::new_v4());

        let hash = 123;
        let data =
            b"test data, we will not gzip it for test brevity since we won't bother unzipping it";
        let res = store.put_chunk(hash, &mut Cursor::new(data));

        head_mock.assert();
        put_mock.assert();
        assert!(res.is_ok());
    }

    #[test]
    fn failing_put_chunk_causes_error() {
        let srv = MockServer::start();
        let head_mock = srv.mock(|when, then| {
            when.method("HEAD");
            then.status(204);
        });
        let put_mock = srv.mock(|when, then| {
            when.method("PUT");
            then.status(400);
        });

        let client = CurlHttpClient::new("0.0.0").unwrap();
        let mut store = super::HttpStore::new(Rc::new(client), srv.base_url(), Uuid::new_v4());

        let hash = 123;
        let data =
            b"test data, we will not gzip it for test brevity since we won't bother unzipping it";
        let res = store.put_chunk(hash, &mut Cursor::new(data));

        head_mock.assert();
        put_mock.assert();
        assert!(res.is_err());
    }

    #[test]
    fn can_get_chunk() {
        let srv = MockServer::start();
        let get_mock = srv.mock(|when, then| {
            when.method("GET");
            then.status(200).body({
                let mut buf = Vec::new();
                let mut encoder = GzEncoder::new(Cursor::new(b"test data"), Compression::none());
                encoder.read_to_end(&mut buf).ok();

                buf
            });
        });

        let client = CurlHttpClient::new("0.0.0").unwrap();
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
    fn missing_get_chunk_causes_error() {
        let srv = MockServer::start();
        let get_mock = srv.mock(|when, then| {
            when.method("GET");
            then.status(404);
        });

        let client = CurlHttpClient::new("0.0.0").unwrap();
        let store = super::HttpStore::new(Rc::new(client), srv.base_url(), Uuid::new_v4());

        let res = store.get_chunk(0x00);

        get_mock.assert();
        assert!(res.is_err());
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

        let client = CurlHttpClient::new("0.0.0").unwrap();
        let mut store = super::HttpStore::new(Rc::new(client), srv.base_url(), Uuid::new_v4());

        let hash = 123;
        let data = b"test data";
        store.put_chunk(hash, &mut Cursor::new(data)).unwrap();

        head_mock.assert();
        put_mock.assert_calls(0);
    }
}
