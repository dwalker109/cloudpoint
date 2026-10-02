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

pub fn load(
    ui_tx: &Sender<UiMsg>,
    modal_tx: &Sender<OpenModalMsg>,
) -> anyhow::Result<(TitleDb, StateDb, InstallHistoryDb)> {
    let (mut title_db, mut state_db) =
        match (TitleDb::open(AppPath::Db), StateDb::open(AppPath::Db)) {
            (Ok(title_db), Ok(state_db)) => (title_db, state_db),
            (Ok(title_db), Err(_)) => {
                modal_tx.send(OpenModalMsg::Refresh).ok();
                let state_db = StateDb::new(AppPath::Db, &title_db, &ui_tx)?;

                (title_db, state_db)
            }
            (Err(_), Ok(mut state_db)) => {
                modal_tx.send(OpenModalMsg::Refresh).ok();
                let title_db = TitleDb::new(AppPath::Db, &ui_tx)?;

                modal_tx.send(OpenModalMsg::Refresh).ok();
                state_db.refresh(true, &title_db, &ui_tx);

                (title_db, state_db)
            }
            (Err(_), Err(_)) => {
                modal_tx.send(OpenModalMsg::Refresh).ok();
                let title_db = TitleDb::new(AppPath::Db, &ui_tx)?;

                modal_tx.send(OpenModalMsg::Refresh).ok();
                let state_db = StateDb::new(AppPath::Db, &title_db, &ui_tx)?;

                (title_db, state_db)
            }
        };

    let mut install_history_db =
        InstallHistoryDb::open(AppPath::Db).or_else(|_| InstallHistoryDb::new(AppPath::Db))?;

    title_db.prune_orphaned();
    state_db.prune_orphaned();
    install_history_db.prune_orphaned();

    Ok((title_db, state_db, install_history_db))
}
