use serde::{Deserialize, Serialize};
use std::{fs, sync::LazyLock};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Settings {
    base_url: String,
    user_key: String,
}

impl Settings {
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn user_key(&self) -> &str {
        &self.user_key
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
