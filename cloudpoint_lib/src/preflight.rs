use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Connect {
    pub srv_ver: semver::Version,
    pub app_update: Option<ClientUpdate>,
    pub srv_update: Option<ServerUpdate>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ClientUpdate {
    pub version: semver::Version,
    pub url: String,
    pub size: usize,
    pub digest: String,
    pub required: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ServerUpdate {
    pub version: semver::Version,
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
