use crate::ctr::CtrSmdh;

pub struct TitleDetails {
    pub title_id: u64,
    pub smdh: CtrSmdh,
    pub has_savedata: bool,
    pub has_extdata: bool,
    pub enabled_savedata: bool,
    pub enabled_extdata: bool,
}

pub enum TitleDataStatus {
    NoData,
    Enabled,
    Disabled,
}

impl TitleDetails {
    pub fn savedata_status(&self) -> TitleDataStatus {
        match (self.has_savedata, self.enabled_savedata) {
            (true, true) => TitleDataStatus::Enabled,
            (true, false) => TitleDataStatus::Disabled,
            (false, true) => unreachable!(),
            (false, false) => TitleDataStatus::Disabled,
        }
    }

    pub fn extdata_status(&self) -> TitleDataStatus {
        match (self.has_extdata, self.enabled_extdata) {
            (true, true) => TitleDataStatus::Enabled,
            (true, false) => TitleDataStatus::Disabled,
            (false, true) => unreachable!(),
            (false, false) => TitleDataStatus::Disabled,
        }
    }
}
