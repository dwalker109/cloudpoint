use serde::{Deserialize, Serialize};

use crate::utils::decode_utf16;

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

pub struct CtrSmdh(Vec<u8>);

impl From<Vec<u8>> for CtrSmdh {
    fn from(buffer: Vec<u8>) -> Self {
        assert_eq!(buffer.len(), 0x36c0);

        Self(buffer)
    }
}

impl CtrSmdh {
    pub fn magic(&self) -> String {
        decode_utf16(&self.0[0x0000..0x0004])
    }

    pub fn version(&self) -> u16 {
        u16::from_le_bytes([self.0[0x0004], self.0[0x0005]])
    }

    pub fn title_short(&self, lang: SmdhLanguage) -> String {
        CtrSmdh::extract_title_field(&self.0, lang.idx(), 0x0008, 0x0080)
    }

    pub fn title_long(&self, lang: SmdhLanguage) -> String {
        CtrSmdh::extract_title_field(&self.0, lang.idx(), 0x0088, 0x0100)
    }

    pub fn title_publisher(&self, lang: SmdhLanguage) -> String {
        CtrSmdh::extract_title_field(&self.0, lang.idx(), 0x0188, 0x0080)
    }

    fn extract_title_field(buffer: &[u8], idx: usize, start: usize, len: usize) -> String {
        let start = start + idx * 0x0200;
        let end = start + len;

        decode_utf16(&buffer[start..end])
    }
}

pub enum SmdhLanguage {
    Japanese,
    English,
    French,
    German,
    Italian,
    Spanish,
    SimplifiedChinese,
    Korean,
    Dutch,
    Portuguese,
    Russian,
    TraditionalChinese,
}

impl SmdhLanguage {
    pub fn idx(&self) -> usize {
        match self {
            SmdhLanguage::Japanese => 0,
            SmdhLanguage::English => 1,
            SmdhLanguage::French => 2,
            SmdhLanguage::German => 3,
            SmdhLanguage::Italian => 4,
            SmdhLanguage::Spanish => 5,
            SmdhLanguage::SimplifiedChinese => 6,
            SmdhLanguage::Korean => 7,
            SmdhLanguage::Dutch => 8,
            SmdhLanguage::Portuguese => 9,
            SmdhLanguage::Russian => 10,
            SmdhLanguage::TraditionalChinese => 11,
        }
    }
}
