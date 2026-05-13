use super::*;
use crate::{
    ctr_title,
    title::{TitleDetails, TitleSyncStatus},
};
use anyhow::Result;
use ctru::services::{am::Am, fs::MediaType};

pub struct TitleDb(Vec<TitleDetails>);

impl TitleDb {
    pub fn build(state_db: &StateDb) -> Result<Self> {
        log::info!("building runtime title db");

        let mut titles = Vec::new();

        let am = Am::new()?;
        let installed_titles = am.title_list(MediaType::Sd)?;
        let installed_apps = installed_titles
            .iter()
            .filter(|t| (t.id() >> 32) as u32 == 0x00040000);

        for title in installed_apps {
            let title_id = title.id();
            let product_code = title.product_code();
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
