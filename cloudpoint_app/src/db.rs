use crate::app::{OpenModalMsg, UiMsg};
use anyhow::Result;
use cloudpoint_lib::db::Persist;
pub use install_history::{InstallHistoryDb, InstallStatus};
pub use state::StateDb;
use std::sync::mpsc::Sender;
pub use title::{TitleDb, TitleDetails};

mod install_history;
mod state;
mod title;

pub fn load(
    ui_tx: &Sender<UiMsg>,
    modal_tx: &Sender<OpenModalMsg>,
) -> Result<(TitleDb, StateDb, InstallHistoryDb)> {
    let (title_db, state_db) = match (TitleDb::open(), StateDb::open()) {
        (Ok(mut title_db), Ok(mut state_db)) => {
            if title_db.is_stale() {
                modal_tx.send(OpenModalMsg::Refresh).ok();

                title_db.refresh(&ui_tx);
                title_db.commit()?;

                state_db.refresh(true, &title_db, &ui_tx);
                state_db.commit()?;

                ui_tx.send(UiMsg::RefreshDone).ok();
            }

            (title_db, state_db)
        }
        (Ok(mut title_db), Err(e)) => {
            log::info!("could not load state db: {e}");

            modal_tx.send(OpenModalMsg::Refresh).ok();

            if title_db.is_stale() {
                title_db.refresh(&ui_tx);
                title_db.commit()?;
            }

            let mut state_db = StateDb::new(&title_db, &ui_tx)?;
            state_db.commit()?;

            ui_tx.send(UiMsg::RefreshDone).ok();

            (title_db, state_db)
        }
        (Err(e), Ok(mut state_db)) => {
            log::info!("could not load title db: {e}");

            modal_tx.send(OpenModalMsg::Refresh).ok();

            let mut title_db = TitleDb::new(&ui_tx)?;
            title_db.commit()?;

            state_db.refresh(true, &title_db, &ui_tx);
            state_db.commit()?;

            ui_tx.send(UiMsg::RefreshDone).ok();

            (title_db, state_db)
        }
        (Err(e1), Err(e2)) => {
            log::info!("could not load title db: {e1}");
            log::info!("could not load state db: {e2}");

            modal_tx.send(OpenModalMsg::Refresh).ok();

            let mut title_db = TitleDb::new(&ui_tx)?;
            title_db.commit()?;

            let mut state_db = StateDb::new(&title_db, &ui_tx)?;
            state_db.commit()?;

            ui_tx.send(UiMsg::RefreshDone).ok();

            (title_db, state_db)
        }
    };

    let install_history_db = match InstallHistoryDb::open() {
        Ok(install_history_db) => install_history_db,
        Err(e) => {
            log::info!("could not load install history db: {e}");

            let mut install_history_db = InstallHistoryDb::new()?;
            install_history_db.commit()?;

            install_history_db
        }
    };

    Ok((title_db, state_db, install_history_db))
}
