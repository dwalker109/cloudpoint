use crate::utils::decode_utf16;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum CtrArchiveId {
    Savedata(u64),
    Extdata(u64),
}

impl std::fmt::Display for CtrArchiveId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CtrArchiveId::Savedata(title_id) => write!(f, "{title_id:016X} savedata"),
            CtrArchiveId::Extdata(extdata_id) => write!(f, "{extdata_id:016X} extdata"),
        }
    }
}

impl From<CtrArchiveId> for PathBuf {
    fn from(value: CtrArchiveId) -> Self {
        match value {
            CtrArchiveId::Savedata(title_id) => PathBuf::from(format!("{title_id:016X}.savedata")),
            CtrArchiveId::Extdata(extdata_id) => {
                PathBuf::from(format!("{extdata_id:016X}.extdata"))
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct CtrMeta {
    required_version: u16,
}

impl CtrMeta {
    pub fn new(required_version: u16) -> Self {
        Self { required_version }
    }

    pub fn required_version(&self) -> u16 {
        self.required_version
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
