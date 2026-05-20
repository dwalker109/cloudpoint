use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::LazyLock,
};
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize)]
pub struct Settings {
    pub base_url: String,
    pub log_level: String,
    pub retain_log_qty: usize,
    pub backup: bool,
}

pub static USER_SETTINGS: LazyLock<Settings> = LazyLock::new(|| {
    config::Config::builder()
        .set_default("base_url", "https://cloudpoint.dwalker.me")
        .unwrap()
        .set_default("log_level", "info")
        .unwrap()
        .set_default("retain_log_qty", 10)
        .unwrap()
        .set_default("backup", true)
        .unwrap()
        .add_source(config::File::from_str(
            &fs::read_to_string(AppPath::Base.join("settings.ini")).unwrap_or_default(),
            config::FileFormat::Ini,
        ))
        .build()
        .unwrap_or_default()
        .try_deserialize::<Settings>()
        .unwrap()
});

pub static USER_KEY: LazyLock<Uuid> = LazyLock::new(|| {
    let path = AppPath::Base.join("user.key");

    if fs::exists(&path).unwrap_or_default() {
        log::info!("using existing user.key");

        let raw = fs::read_to_string(&path).expect("userkey should be accessible");
        let userkey = Uuid::try_parse(&raw).expect("userkey should be a uuid");

        userkey
    } else {
        log::info!("creating new user.key");

        let userkey = Uuid::new_v4();
        let mut buf = Uuid::encode_buffer();
        fs::write(&path, userkey.as_hyphenated().encode_lower(&mut buf))
            .expect("userkey should be writable");

        userkey
    }
});

#[derive(Debug)]
pub enum AppPath {
    Base,
    Db,
    Backup,
    Log,
}

impl AsRef<Path> for AppPath {
    fn as_ref(&self) -> &Path {
        match self {
            AppPath::Base => Path::new("sdmc:/3ds/Cloudpoint"),
            AppPath::Db => Path::new("sdmc:/3ds/Cloudpoint/db"),
            AppPath::Backup => Path::new("sdmc:/3ds/Cloudpoint/backup"),
            AppPath::Log => Path::new("sdmc:/3ds/Cloudpoint/log"),
        }
    }
}

impl AppPath {
    pub fn join(&self, path: impl AsRef<Path>) -> PathBuf {
        self.as_ref().join(path)
    }
}
