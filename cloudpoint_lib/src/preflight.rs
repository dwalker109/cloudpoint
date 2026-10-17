use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum Update {
    Client {
        to_version: semver::Version,
        required: bool,
        url: String,
    },
    Server {
        to_version: semver::Version,
        required: bool,
    },
}

impl Update {
    pub fn is_required(&self) -> bool {
        match self {
            Update::Client { required, .. } => *required,
            Update::Server { required, .. } => *required,
        }
    }
}
