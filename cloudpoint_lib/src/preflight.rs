use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum Connect {
    Ready,
    ClientUpdate {
        to_version: String,
        url: String,
        required: bool,
    },
    ServerUpdate {
        to_version: String,
        required: bool,
    },
}
