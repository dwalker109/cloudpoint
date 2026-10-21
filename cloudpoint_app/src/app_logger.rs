use crate::config::{AppPath, USER_SETTINGS};
use anyhow::Result;
use chrono::NaiveDateTime;
use flexi_logger::LoggerHandle;
use std::fs;

pub struct AppLogger(LoggerHandle);

impl AppLogger {
    pub fn new() -> Result<Self> {
        let h = flexi_logger::Logger::try_with_str(&USER_SETTINGS.log_level)?
            .log_to_file(
                flexi_logger::FileSpec::default()
                    .basename("cloudpoint")
                    .directory(AppPath::Log.as_ref()),
            )
            .start()?;

        Ok(Self(h))
    }
}

impl Drop for AppLogger {
    fn drop(&mut self) {
        log::info!("cleaning up logfiles");

        const DT_LEN: usize = 19;
        const DT_PATTERN: &str = "%Y-%m-%d_%H-%M-%S";

        let mut all_logs = fs::read_dir(AppPath::Log)
            .expect("log dir should be available")
            .flatten()
            .filter_map(|f| {
                let is_file = f.file_type().map_or(false, |f| f.is_file());

                if is_file {
                    let path = f.path();
                    let stem = path.file_stem()?.to_str()?;
                    let dt_str = &stem.get(stem.len() - DT_LEN..)?;
                    let dt = NaiveDateTime::parse_from_str(dt_str, DT_PATTERN).ok()?;

                    Some((f.path(), dt))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        all_logs.sort_by(|(_, dt_a), (_, dt_b)| dt_b.cmp(&dt_a));

        for (p, _) in all_logs.iter().skip(USER_SETTINGS.retain_log_qty) {
            log::debug!("deleting {}", p.display());
            fs::remove_file(p).ok();
        }

        log::info!("flushing logs");
        self.0.flush();
    }
}
