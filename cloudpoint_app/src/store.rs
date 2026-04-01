use anyhow::anyhow;
use chunktree::store::{StoreError, StoreRead, StoreWrite};
use cloudpoint_lib::http::CurlHttpClient;
use std::{
    io::{self, Cursor, Read},
    sync::Arc,
};

pub struct HttpStore(pub Arc<CurlHttpClient>, pub String);

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
