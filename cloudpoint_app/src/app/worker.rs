use super::*;
use crate::{
    config::{APP_VER, AppPath},
    ctr_nwm::ForceWlan,
    db::{InstallHistoryDb, StateDb, TitleDb},
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
    shutdown_rx: Receiver<()>,
    ui_tx: Sender<UiMsg>,
    modal_tx: Sender<OpenModalMsg>,
) -> Result<()> {
    let _wlan = ForceWlan::new()?;

    let mut state_db = StateDb::open(AppPath::Db)
        .or_else(|_| {
            modal_tx.send(OpenModalMsg::Refresh).ok();
            StateDb::new(AppPath::Db, &ui_tx)
        })
        .expect("state db must be available");

    let mut title_db = TitleDb::open(AppPath::Db)
        .or_else(|_| {
            modal_tx.send(OpenModalMsg::Refresh).ok();
            TitleDb::new(AppPath::Db, &state_db, &ui_tx)
        })
        .expect("title db must be available");

    let mut install_history_db = InstallHistoryDb::open(AppPath::Db)
        .or_else(|_| InstallHistoryDb::new(AppPath::Db))
        .expect("install history db must be available");

    let client = Rc::new(CurlHttpClient::new(&APP_VER).expect("curl client must be available"));

    state_db.prune_orphaned()?;
    title_db.prune_orphaned()?;
    // install_db is *not* pruned, we want that to survive title and OS reinstalls

    ui_tx
        .send(UiMsg::RefreshDone {
            qty_sync_states: state_db.qty_auto(),
            titles: title_db.titles_sorted_vec(),
        })
        .ok();

    loop {
        match task_rx.recv() {
            Ok(TaskMsg::Refresh) => {
                modal_tx.send(OpenModalMsg::Refresh).ok();
                state_db.add_missing(true, &ui_tx)?;
                title_db.add_all(&state_db, &ui_tx)?;
                ui_tx
                    .send(UiMsg::RefreshDone {
                        qty_sync_states: state_db.qty_auto(),
                        titles: title_db.titles_sorted_vec(),
                    })
                    .ok();
            }
            Ok(TaskMsg::Toggle(title_id)) => {
                state_db.process_sync_items_for_title(title_id, false)?;
                state_db.toggle_auto_sync_for_title(title_id)?;
                title_db.refresh_shared_extdata_linked_titles(title_id, &state_db)?;
                ui_tx
                    .send(UiMsg::RefreshDone {
                        qty_sync_states: state_db.qty_auto(),
                        titles: title_db.titles_sorted_vec(),
                    })
                    .ok();
            }
            Ok(TaskMsg::SyncAuto) => {
                match sync::run(
                    state_db.states_mut().filter(|s| s.auto_enabled),
                    &shutdown_rx,
                    ui_tx.clone(),
                    modal_tx.clone(),
                    &client,
                    &mut install_history_db,
                ) {
                    Ok(_) => {
                        ui_tx
                            .send(UiMsg::SyncDone {
                                result: "Sync completed".into(),
                                message: chrono::Utc::now().format("%Y-%m-%d %H:%M").to_string(),
                            })
                            .ok();
                    }
                    Err(e) => {
                        ui_tx
                            .send(UiMsg::SyncDone {
                                result: "Sync failed".into(),
                                message: chrono::Utc::now().format("%Y-%m-%d %H:%M").to_string(),
                            })
                            .ok();
                        modal_tx
                            .send(OpenModalMsg::Error {
                                label: "Error".into(),
                                message: e.to_string(),
                            })
                            .ok();
                    }
                };
            }
            Ok(TaskMsg::SyncTargeted(title_id)) => {
                match sync::run(
                    state_db
                        .states_mut()
                        .filter(|s| s.via_title_ids.contains(&title_id)),
                    &shutdown_rx,
                    ui_tx.clone(),
                    modal_tx.clone(),
                    &client,
                    &mut install_history_db,
                ) {
                    Ok(_) => {
                        ui_tx
                            .send(UiMsg::SyncDone {
                                result: "Sync completed".into(),
                                message: chrono::Utc::now().format("%Y-%m-%d %H:%M").to_string(),
                            })
                            .ok();
                    }
                    Err(e) => {
                        ui_tx
                            .send(UiMsg::SyncDone {
                                result: "Sync failed".into(),
                                message: chrono::Utc::now().format("%Y-%m-%d %H:%M").to_string(),
                            })
                            .ok();
                        modal_tx
                            .send(OpenModalMsg::Error {
                                label: "Error".into(),
                                message: e.to_string(),
                            })
                            .ok();
                    }
                };
                ui_tx
                    .send(UiMsg::RefreshDone {
                        qty_sync_states: state_db.qty_auto(),
                        titles: title_db.titles_sorted_vec(),
                    })
                    .ok();
            }
            Ok(TaskMsg::LinkHost) => {
                if let Err(e) = link::host(&ui_tx, &modal_tx) {
                    log::error!("errored during user key share as host: {e}");
                    ui_tx
                        .send(UiMsg::LinkUpdate {
                            state: link::LinkState::Failed,
                        })
                        .ok();
                }
            }
            Ok(TaskMsg::LinkClient) => {
                if let Err(e) = link::client(&ui_tx, &modal_tx) {
                    log::error!("errored during user key share as client: {e}");
                    ui_tx
                        .send(UiMsg::LinkUpdate {
                            state: link::LinkState::Failed,
                        })
                        .ok();
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
