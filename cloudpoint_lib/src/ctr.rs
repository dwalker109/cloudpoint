use chunktree::version::Meta;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum CtrArchiveKind {
    Savedata,
    Extdata,
}

impl std::fmt::Display for CtrArchiveKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CtrArchiveKind::Savedata => write!(f, "savedata"),
            CtrArchiveKind::Extdata => write!(f, "extdata"),
        }
    }
}

impl TryFrom<&str> for CtrArchiveKind {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "savedata" => Ok(CtrArchiveKind::Savedata),
            "extdata" => Ok(CtrArchiveKind::Extdata),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct CtrMeta {
    title_version: u16,
}

impl CtrMeta {
    pub fn new(title_version: u16) -> Self {
        Self { title_version }
    }

    pub fn title_version(&self) -> u16 {
        self.title_version
    }
}

impl Meta for CtrMeta {}
