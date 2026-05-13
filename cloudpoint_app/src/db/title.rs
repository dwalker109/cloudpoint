use super::*;
use crate::{
    ctr_title::{self, SD_APP_TITLES},
    title::{TitleDetails, TitleSyncStatus},
};
use anyhow::Result;

pub struct TitleDb(Vec<TitleDetails>);

impl TitleDb {
    pub fn build(state_db: &StateDb) -> Result<Self> {
        log::info!("building runtime title db");

        let mut titles = Vec::new();

        for title in SD_APP_TITLES.iter() {
            let title_id = title.title_id;
            let product_code = &title.product_code;
            let smdh = ctr_title::smdh(title_id)?;

            log::info!("processing {title_id:016X}");

            let title = TitleDetails::new(title_id, &product_code, smdh, &state_db);

            if title.savedata_sync_status != TitleSyncStatus::Unavailable
                || title.extdata_sync_status != TitleSyncStatus::Unavailable
            {
                log::debug!("added {title_id:016X}");
                titles.push(title);
            } else {
                log::debug!("ignored {title_id:016X}, has no save or extdata");
            }
        }

        titles.sort_by_key(|t| {
            t.smdh
                .title_short(cloudpoint_lib::ctr::SmdhLanguage::English)
        });

        Ok(Self(titles))
    }

    pub fn total_titles(&self) -> usize {
        self.0.len()
    }

    pub fn titles(&self) -> impl Iterator<Item = &TitleDetails> {
        self.0.iter()
    }
}
