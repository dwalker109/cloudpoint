use serde::{Deserialize, Serialize};
use std::{fs, sync::LazyLock};
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize)]
pub struct Settings {
    pub base_url: String,
    pub log: String,
    pub backup: bool,
}

pub static SETTINGS: LazyLock<Settings> = LazyLock::new(|| {
    config::Config::builder()
        .set_default("base_url", "http://192.168.1.45:5000")
        .unwrap()
        .set_default("log", "off")
        .unwrap()
        .set_default("backup", true)
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

pub static USER_KEY: LazyLock<Uuid> = LazyLock::new(|| {
    let path = "sdmc:/3ds/Cloudpoint/user.key";

    if fs::exists(path).unwrap_or_default() {
        let raw = fs::read_to_string(&path).expect("userkey should be accessible");
        let userkey = Uuid::try_parse(&raw).expect("userkey should be a uuid");

        userkey
    } else {
        let userkey = Uuid::new_v4();
        let mut buf = Uuid::encode_buffer();
        fs::write(&path, userkey.as_hyphenated().encode_lower(&mut buf))
            .expect("userkey should be writable");

        userkey
    }
});
