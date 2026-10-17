use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum Connect {
    Ready,
    ClientUpdate {
        to_version: semver::Version,
        required: bool,
        url: String,
    },
    ServerUpdate {
        to_version: semver::Version,
        required: bool,
    },
}
