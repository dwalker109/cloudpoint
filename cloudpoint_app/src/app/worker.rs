use super::*;
use crate::{
    config::AppPath,
    db::{StateDb, TitleDb},
    link, sync,
};
use anyhow::Result;
use cloudpoint_lib::http::CurlHttpClient;
use std::{
    rc::Rc,
    sync::mpsc::{Receiver, Sender},
};

pub fn worker_thread(
    task_rx: Receiver<TaskMsg>,
    ui_tx: Sender<UiMsg>,
    modal_tx: Sender<OpenModalMsg>,
) -> Result<()> {
    let (mut state_db, mut title_db) = {
        if let (Ok(state_db), Ok(title_db)) =
            (StateDb::open(AppPath::Db), TitleDb::open(AppPath::Db))
        {
            log::debug!("state db and title db loaded from disk on startup");
            (state_db, title_db)
        } else {
            modal_tx.send(OpenModalMsg::Refresh)?;
            let state_db = StateDb::new(AppPath::Db, &ui_tx).expect("state db should be creatable");
            let title_db =
                TitleDb::new(AppPath::Db, &state_db, &ui_tx).expect("title db should be creatable");

            log::debug!("state db and title db recreated on startup");
            (state_db, title_db)
        }
    };

    ui_tx.send(UiMsg::RefreshDone {
        qty_sync_states: state_db.qty_auto(),
        titles: title_db.titles_sorted_vec(),
    })?;

    let client = Rc::new(CurlHttpClient::new().expect("curl client should be available"));

    loop {
        match task_rx.recv() {
            Ok(TaskMsg::Refresh) => {
                modal_tx.send(OpenModalMsg::Refresh)?;
                state_db.refresh(true, &ui_tx)?;
                title_db.refresh(&state_db, &ui_tx)?;
                ui_tx.send(UiMsg::RefreshDone {
                    qty_sync_states: state_db.qty_auto(),
                    titles: title_db.titles_sorted_vec(),
                })?;
            }
            Ok(TaskMsg::Toggle(title_id)) => {
                state_db.refresh_for_title_id(title_id, false)?;
                state_db.toggle_for_title_id(title_id)?;
                title_db.refresh_links(title_id, &state_db)?;
                ui_tx.send(UiMsg::RefreshDone {
                    qty_sync_states: state_db.qty_auto(),
                    titles: title_db.titles_sorted_vec(),
                })?;
            }
            Ok(TaskMsg::SyncAuto) => {
                match sync::run(
                    state_db.states_mut().filter(|s| s.auto_enabled),
                    ui_tx.clone(),
                    modal_tx.clone(),
                    &client,
                ) {
                    Ok(_) => {
                        ui_tx.send(UiMsg::SyncDone {
                            result: "Sync completed".into(),
                            message: chrono::Utc::now().format("%Y-%m-%d %H:%M").to_string(),
                        })?;
                    }
                    Err(e) => {
                        ui_tx.send(UiMsg::SyncDone {
                            result: "Sync failed".into(),
                            message: chrono::Utc::now().format("%Y-%m-%d %H:%M").to_string(),
                        })?;
                        modal_tx.send(OpenModalMsg::Error {
                            label: "Error".into(),
                            message: e.to_string(),
                        })?;
                    }
                };
            }
            Ok(TaskMsg::SyncTargeted(title_id)) => {
                state_db.refresh_for_title_id(title_id, false)?;
                title_db.refresh_links(title_id, &state_db)?;
                match sync::run(
                    state_db
                        .states_mut()
                        .filter(|s| s.via_title_ids.contains(&title_id)),
                    ui_tx.clone(),
                    modal_tx.clone(),
                    &client,
                ) {
                    Ok(_) => {
                        ui_tx.send(UiMsg::SyncDone {
                            result: "Sync completed".into(),
                            message: chrono::Utc::now().format("%Y-%m-%d %H:%M").to_string(),
                        })?;
                    }
                    Err(e) => {
                        ui_tx.send(UiMsg::SyncDone {
                            result: "Sync failed".into(),
                            message: chrono::Utc::now().format("%Y-%m-%d %H:%M").to_string(),
                        })?;
                        modal_tx.send(OpenModalMsg::Error {
                            label: "Error".into(),
                            message: e.to_string(),
                        })?;
                    }
                };
                ui_tx.send(UiMsg::RefreshDone {
                    qty_sync_states: state_db.qty_auto(),
                    titles: title_db.titles_sorted_vec(),
                })?;
            }
            Ok(TaskMsg::LinkHost) => {
                if let Err(e) = link::host(&ui_tx, &modal_tx) {
                    log::error!("errored during user key share as host: {e}");
                    ui_tx.send(UiMsg::LinkHostDone { success: false })?;
                }
            }
            Ok(TaskMsg::LinkClient) => {
                if let Err(e) = link::client(&ui_tx, &modal_tx) {
                    log::error!("errored during user key share as client: {e}");
                    ui_tx.send(UiMsg::LinkClientDone { success: false })?;
                }
            }
            Err(e) => {
                log::info!("worker thread exiting, this is probably normal: {e}");
                break;
            }
        }
    }

    Ok(())
}
