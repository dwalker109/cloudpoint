use anyhow::{Result, bail};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::PathBuf,
};

pub trait Migrate: DeserializeOwned {
    const VERSION_ID: u16;

    type Previous: Migrate;

    fn decode(ver: u16, buf: &[u8]) -> Result<Self> {
        if ver == Self::VERSION_ID {
            log::debug!(
                "found db with version id {ver} and {} bytes of data, decoding",
                buf.len()
            );
            return Ok(postcard::from_bytes(buf)?);
        }

        let prev = Self::Previous::decode(ver, buf)?;

        log::debug!(
            "migrating from version id {} to {}",
            Self::Previous::VERSION_ID,
            Self::VERSION_ID
        );

        Self::migrate(prev)
    }

    fn migrate(prev: Self::Previous) -> Result<Self>;
}

#[derive(Deserialize)]
pub struct MigrateRoot;

impl Migrate for MigrateRoot {
    const VERSION_ID: u16 = 0;

    type Previous = Self;

    fn decode(_ver: u16, _buf: &[u8]) -> Result<Self> {
        bail!("reached root migration without matching, cannot decode")
    }

    fn migrate(_prev: Self::Previous) -> Result<Self> {
        unreachable!()
    }
}

pub trait Persist: Migrate + Serialize + DeserializeOwned {
    fn path() -> PathBuf;

    fn open() -> Result<Self> {
        let path = Self::path();

        log::debug!("loading {} from disk", path.display());

        read(&fs::read(path)?)
    }

    fn commit(&mut self) -> Result<()> {
        let path = Self::path();

        log::debug!("saving {} to disk", path.display());

        #[cfg(feature = "db_debug")]
        {
            std::fs::write(
                path.with_added_extension(format!("debug.v{}.ron", Self::VERSION_ID)),
                ron::ser::to_string_pretty(&self, ron::ser::PrettyConfig::default())?,
            )?;
        }

        write(&mut BufWriter::new(File::create(&path)?), self)
    }
}

const HEADER_BYTES: [u8; 4] = *b"CPDB";

fn read<T: Persist>(buf: &[u8]) -> Result<T> {
    let (ver, buf) = match buf.split_first_chunk::<6>() {
        Some((head, rest)) if head[..4] == HEADER_BYTES => {
            (u16::from_le_bytes(head[4..].try_into()?), rest)
        }
        _ => (0, buf),
    };

    T::decode(ver, buf)
}

fn write<T: Persist>(mut w: &mut impl Write, data: &T) -> Result<()> {
    w.write_all(&HEADER_BYTES)?;
    w.write_all(&T::VERSION_ID.to_le_bytes())?;
    postcard::to_io(data, &mut w)?;
    w.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[test]
    fn can_migrate_full_chain() {
        #[derive(Deserialize, Serialize)]
        struct M0 {
            foo: String,
        }

        impl Persist for M0 {
            fn path() -> PathBuf {
                unreachable!()
            }
        }

        impl Migrate for M0 {
            const VERSION_ID: u16 = 0;

            type Previous = MigrateRoot;

            fn migrate(_prev: Self::Previous) -> Result<Self> {
                unreachable!()
            }
        }

        #[derive(Deserialize, Serialize)]
        struct M1 {
            foo: String,
            bar: u8,
        }

        impl Migrate for M1 {
            const VERSION_ID: u16 = 1;

            type Previous = M0;

            fn migrate(prev: Self::Previous) -> Result<Self> {
                Ok(Self {
                    foo: prev.foo.to_ascii_uppercase(),
                    bar: b'A',
                })
            }
        }

        #[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
        struct M2 {
            foo: String,
            bar: bool,
        }

        impl Persist for M2 {
            fn path() -> PathBuf {
                unreachable!()
            }
        }

        impl Migrate for M2 {
            const VERSION_ID: u16 = 2;

            type Previous = M1;

            fn migrate(prev: Self::Previous) -> Result<Self> {
                Ok(Self {
                    foo: format!("...{}...", prev.foo),
                    bar: u8::is_ascii_alphanumeric(&prev.bar),
                })
            }
        }

        let first = M0 { foo: "foo".into() };
        let expected = M2 {
            foo: "...FOO...".into(),
            bar: true,
        };

        let mut buf = Vec::new();
        write(&mut buf, &first).unwrap();
        let actual: M2 = read(&buf).unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn invalid_migration_chain_errors_appropriately() {
        #[derive(Deserialize)]
        struct M0;

        impl Migrate for M0 {
            const VERSION_ID: u16 = u16::MAX;

            type Previous = MigrateRoot;

            fn migrate(_prev: Self::Previous) -> Result<Self> {
                unreachable!()
            }
        }

        let m0 = M0::decode(0, &[]);
        assert!(m0.is_err());
    }
}
