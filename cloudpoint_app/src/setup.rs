use crate::config::AppPath;
use anyhow::{Context, Result};
use std::fs;

pub fn sdmc() -> Result<()> {
    let paths = [AppPath::Base, AppPath::Db, AppPath::Log];
    for p in paths {
        fs::create_dir_all(&p).with_context(|| {
            format!("fatal: failed to create directory {}", p.as_ref().display())
        })?;
    }

    log::debug!("Created paths");

    Ok(())
}
