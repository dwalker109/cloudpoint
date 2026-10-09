use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct UpgradeHint {
    version: String,
    uri: String,
    optional: bool,
}
