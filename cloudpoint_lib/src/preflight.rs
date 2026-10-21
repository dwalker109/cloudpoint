use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Connect {
    pub srv_ver: semver::Version,
    pub app_update: Option<ClientUpdate>,
    pub srv_update: Option<ServerUpdate>,
}

#[derive(Serialize, Deserialize)]
pub struct ClientUpdate {
    pub version: semver::Version,
    pub url: String,
    pub required: bool,
}

#[derive(Serialize, Deserialize)]
pub struct ServerUpdate {
    pub version: semver::Version,
    pub url: String,
    pub required: bool,
}

impl Default for Connect {
    fn default() -> Self {
        Self {
            srv_ver: semver::Version::new(0, 0, 0),
            app_update: None,
            srv_update: None,
        }
    }
}
