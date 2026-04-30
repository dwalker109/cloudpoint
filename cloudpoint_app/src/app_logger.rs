use std::fs;

use anyhow::Result;
use flexi_logger::LoggerHandle;

use crate::config::{AppPath, USER_SETTINGS};

pub struct AppLogger(LoggerHandle);

impl AppLogger {
    pub fn new() -> Result<Self> {
        let h = flexi_logger::Logger::try_with_str(&USER_SETTINGS.log_level)?
            .log_to_file(flexi_logger::FileSpec::default().directory(AppPath::Log.as_ref()))
            .start()?;

        Ok(Self(h))
    }
}

impl Drop for AppLogger {
    fn drop(&mut self) {
        self.0.flush();

        let mut all_logs = fs::read_dir(AppPath::Log)
            .expect("log dir should be available")
            .flatten()
            .filter_map(|f| {
                let is_file = f.file_type().map_or(false, |f| f.is_file());

                if is_file { Some(f.path()) } else { None }
            })
            .collect::<Vec<_>>();

        all_logs.sort();
        all_logs.reverse();

        for f in all_logs.iter().skip(USER_SETTINGS.retain_log_qty) {
            fs::remove_file(f).ok();
        }
    }
}
