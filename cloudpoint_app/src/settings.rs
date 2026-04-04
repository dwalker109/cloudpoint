use serde::{Deserialize, Serialize};
use std::{fs, sync::LazyLock};

#[derive(Debug, Deserialize, Serialize)]
pub struct Settings {
    pub base_url: String,
    pub user_key: String,
    pub log: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            user_key: String::new(),
            log: String::from("off"),
        }
    }
}

pub static SETTINGS: LazyLock<Settings> = LazyLock::new(|| {
    config::Config::builder()
        .add_source(config::File::from_str(
            &fs::read_to_string("sdmc:/3ds/Cloudpoint/settings.ini").unwrap_or_default(),
            config::FileFormat::Ini,
        ))
        .build()
        .unwrap_or_default()
        .try_deserialize::<Settings>()
        .unwrap_or_default()
});
