use anyhow::anyhow;
use chunktree::store::{StoreError, StoreRead, StoreWrite};
use std::io::{self, Cursor, Read};

pub struct HttpStore(pub String);

impl StoreRead for HttpStore {
    fn get_chunk(&self, hash: u64) -> Result<impl Read, StoreError> {
        let res = minreq::get(format!("{}/chunks/{}", self.0, hash))
            .send()
            .map_err(|err| {
                io::Error::new(
                    io::ErrorKind::Other,
                    anyhow!("failed to get chunk {:?}, {}", hash, err),
                )
            })?;

        Ok(Cursor::new(res.into_bytes()))
    }
}

impl StoreWrite for HttpStore {
    fn put_chunk(&mut self, hash: u64, data: &mut (impl Read + ?Sized)) -> Result<(), StoreError> {
        let url = format!("{}/chunks/{}", self.0, hash);

        let should_upload = minreq::head(url)
            .send()
            .map_err(|err| {
                io::Error::new(
                    io::ErrorKind::Other,
                    anyhow!("failed to check existence of chunk {:?}, {}", hash, err),
                )
            })?
            .status_code
            != 200;

        let mut body = Vec::new();
        data.read_to_end(&mut body)?;

        if should_upload {
            minreq::put(format!("{}/chunks/{}", self.0, hash))
                .with_body(body)
                .send()
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
