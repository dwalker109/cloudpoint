use serde::{Deserialize, Serialize};
use std::{fs, sync::LazyLock};

#[derive(Debug, Deserialize, Serialize)]
pub struct Settings {
    pub base_url: String,
    pub user_key: String,
    pub log: String,
}

pub static SETTINGS: LazyLock<Settings> = LazyLock::new(|| {
    config::Config::builder()
        .set_default("base_url", "http://192.168.1.45:8080")
        .unwrap()
        .set_default("user_key", "foobarbaz")
        .unwrap()
        .set_default("log", "off")
        .unwrap()
        .add_source(config::File::from_str(
            &fs::read_to_string("sdmc:/3ds/Cloudpoint/settings.ini").unwrap_or_default(),
            config::FileFormat::Ini,
        ))
        .build()
        .unwrap_or_default()
        .try_deserialize::<Settings>()
        .unwrap()
});
