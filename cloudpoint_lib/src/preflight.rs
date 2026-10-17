use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Connect {
    pub srv_ver: semver::Version,
    pub app_update: Option<Update>,
    pub srv_update: Option<Update>,
}

#[derive(Serialize, Deserialize)]
pub struct Update {
    pub version: semver::Version,
    pub url: String,
    pub required: bool,
}
