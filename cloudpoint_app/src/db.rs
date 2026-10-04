use std::sync::mpsc::Sender;

pub use install_history::{InstallHistoryDb, InstallStatus};
pub use state::StateDb;
pub use title::{TitleDb, TitleDetails};

use crate::{
    app::{OpenModalMsg, UiMsg},
    config::AppPath,
};

mod install_history;
mod state;
mod title;

pub static COMMIT_MSG: &'static str = "should commit db to sd card";

pub fn load(
    ui_tx: &Sender<UiMsg>,
    modal_tx: &Sender<OpenModalMsg>,
) -> anyhow::Result<(TitleDb, StateDb, InstallHistoryDb)> {
    let (title_db, state_db) = match (TitleDb::open(AppPath::Db), StateDb::open(AppPath::Db)) {
        (Ok(mut title_db), Ok(mut state_db)) => {
            if title_db.is_stale() {
                modal_tx.send(OpenModalMsg::Refresh).ok();
                title_db.refresh(&ui_tx);
                title_db.commit()?;

                modal_tx.send(OpenModalMsg::Refresh).ok();
                state_db.refresh(true, &title_db, &ui_tx);
                state_db.commit()?;
            }

            (title_db, state_db)
        }
        (Ok(mut title_db), Err(_)) => {
            if title_db.is_stale() {
                modal_tx.send(OpenModalMsg::Refresh).ok();
                title_db.refresh(&ui_tx);
                title_db.commit()?;
            }

            modal_tx.send(OpenModalMsg::Refresh).ok();
            let mut state_db = StateDb::new(AppPath::Db, &title_db, &ui_tx)?;
            state_db.commit()?;

            (title_db, state_db)
        }
        (Err(_), Ok(mut state_db)) => {
            modal_tx.send(OpenModalMsg::Refresh).ok();
            let mut title_db = TitleDb::new(AppPath::Db, &ui_tx)?;
            title_db.commit()?;

            modal_tx.send(OpenModalMsg::Refresh).ok();
            state_db.refresh(true, &title_db, &ui_tx);
            state_db.commit()?;

            (title_db, state_db)
        }
        (Err(_), Err(_)) => {
            modal_tx.send(OpenModalMsg::Refresh).ok();
            let mut title_db = TitleDb::new(AppPath::Db, &ui_tx)?;
            title_db.commit()?;

            modal_tx.send(OpenModalMsg::Refresh).ok();
            let mut state_db = StateDb::new(AppPath::Db, &title_db, &ui_tx)?;
            state_db.commit()?;

            (title_db, state_db)
        }
    };

    let install_history_db = match InstallHistoryDb::open(AppPath::Db) {
        Ok(install_history_db) => install_history_db,
        Err(_) => {
            let mut install_history_db = InstallHistoryDb::new(AppPath::Db)?;
            install_history_db.commit()?;

            install_history_db
        }
    };

    Ok((title_db, state_db, install_history_db))
}
