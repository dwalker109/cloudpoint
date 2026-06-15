use anyhow::Context;
use sqlx::PgPool;
use std::{env, fs, path::PathBuf, str::FromStr};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    HexU128,
    services::{chunk, version},
};

pub async fn run(db_pool: &PgPool) -> anyhow::Result<()> {
    let root_dir = PathBuf::from(env::var("V0_IMPORT")?);

    info!(root_dir = root_dir.to_str(), "importing v0 DUFS data");

    let mut chunks_count = (0_usize, 0_usize);
    let mut versions_count = (0_usize, 0_usize);

    for entry in fs::read_dir(root_dir.join("sync")).context("reading top level \"sync\" dir")? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let user_dir = entry.path();
            let user_key = Uuid::from_str(&user_dir.file_name().unwrap().to_string_lossy())
                .context("parsing user dir to UUID")?;

            info!(
                user_dir = user_dir.to_str(),
                user_key = user_key.to_string(),
                "processing user_key dir"
            );

            let chunks_path = user_dir.join("chunks");
            if !fs::exists(&chunks_path)? {
                continue;
            };

            for entry in fs::read_dir(chunks_path)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    let shard_dir = entry.path();

                    for entry in fs::read_dir(shard_dir)? {
                        let entry = entry?;
                        if entry.file_type()?.is_file() {
                            let chunk_cid = HexU128::from(entry.file_name().to_str().unwrap());
                            let chunk_file = entry.path();
                            let chunk_body = fs::read(&chunk_file)?;

                            info!(
                                chunk_cid = chunk_cid.to_string(),
                                chunk_file = chunk_file.to_str(),
                                "processing chunk file"
                            );

                            match chunk::validate(&chunk_cid, &chunk_body) {
                                Ok(len) => {
                                    chunk::put(&user_key, &chunk_cid, &chunk_body, len, db_pool)
                                        .await?;

                                    chunks_count.0 += 1;
                                    info!("processed chunk file")
                                }
                                Err(err) => {
                                    chunks_count.1 += 1;
                                    warn!(err, "skipped chunk file")
                                }
                            }
                        }
                    }
                }
            }

            let sync_items_path = user_dir.join("archives");
            if !fs::exists(&sync_items_path)? {
                continue;
            };

            for entry in fs::read_dir(sync_items_path)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    let si_name = entry.file_name().to_owned().into_string().unwrap();
                    let si_dir = entry.path();

                    info!(si_dir = si_dir.to_str(), "processing sync item dir");

                    for entry in fs::read_dir(si_dir)? {
                        let entry = entry?;
                        if entry.file_type()?.is_file() {
                            let ver_cid = HexU128::from(entry.file_name().to_str().unwrap());
                            let ver_file = entry.path();
                            let ver_body = fs::read(&ver_file)?;

                            info!(
                                sync_item = si_name,
                                ver_cid = ver_cid.to_string(),
                                ver_file = ver_file.to_str(),
                                "processing version file"
                            );

                            match version::validate(&ver_cid, &ver_body) {
                                Ok(()) => {
                                    version::put(&user_key, &si_name, &ver_cid, &ver_body, db_pool)
                                        .await?;

                                    versions_count.0 += 1;
                                    info!("processed version file")
                                }
                                Err(err) => {
                                    versions_count.1 += 1;
                                    warn!(err, "skipped chunk file")
                                }
                            }
                        }
                    }
                }
            }
            info!(
                chunks_ok = chunks_count.0,
                chunks_err = chunks_count.1,
                versions_ok = versions_count.0,
                versions_err = versions_count.1,
                "import finished"
            );
        }
    }

    Ok(())
}
