use super::*;
use crate::{
    config::AppPath,
    db::{InstallHistoryDb, StateDb, TitleDb},
    link, sync,
};
use anyhow::Result;
use itertools::Itertools;
use std::{
    sync::mpsc::{Receiver, Sender},
    time::Instant,
};

pub fn worker_thread(
    task_rx: Receiver<TaskMsg>,
    shutdown_rx: Receiver<()>,
    ui_tx: Sender<UiMsg>,
    modal_tx: Sender<OpenModalMsg>,
) -> Result<()> {
    modal_tx.send(OpenModalMsg::Connect).ok();
    sync::connect(&shutdown_rx, &ui_tx)?;

    let (mut title_db, mut state_db) =
        match (TitleDb::open(AppPath::Db), StateDb::open(AppPath::Db)) {
            (Ok(title_db), Ok(state_db)) => (title_db, state_db),
            (Ok(title_db), Err(_)) => {
                modal_tx.send(OpenModalMsg::Refresh).ok();
                let state_db = StateDb::new(AppPath::Db, &title_db, &ui_tx)
                    .expect("state db must be available");

                (title_db, state_db)
            }
            (Err(_), Ok(mut state_db)) => {
                modal_tx.send(OpenModalMsg::Refresh).ok();
                let title_db =
                    TitleDb::new(AppPath::Db, &ui_tx).expect("title db must be available");

                modal_tx.send(OpenModalMsg::Refresh).ok();
                state_db.refresh(true, &title_db, &ui_tx);

                (title_db, state_db)
            }
            _ => {
                modal_tx.send(OpenModalMsg::Refresh).ok();
                let title_db =
                    TitleDb::new(AppPath::Db, &ui_tx).expect("title db must be available");

                modal_tx.send(OpenModalMsg::Refresh).ok();
                let state_db = StateDb::new(AppPath::Db, &title_db, &ui_tx)
                    .expect("state db must be available");

                (title_db, state_db)
            }
        };

    let mut install_history_db = InstallHistoryDb::open(AppPath::Db)
        .or_else(|_| InstallHistoryDb::new(AppPath::Db))
        .expect("install history db must be available");

    let client = Rc::new(CurlHttpClient::new(&APP_VER).expect("curl client must be available"));

    title_db.prune_orphaned();
    state_db.prune_orphaned();
    // install_db is *not* pruned, we want that to survive title and OS reinstalls

    ui_tx
        .send(UiMsg::Ready {
            titles: title_db.titles_sorted_vec(),
            sync_states: state_db.states_hashmap(),
            qty_auto: state_db.qty_auto(),
        })
        .ok();

    loop {
        match task_rx.recv() {
            Ok(TaskMsg::Refresh) => {
                modal_tx.send(OpenModalMsg::Refresh).ok();
                title_db.refresh(&ui_tx);
                state_db.refresh(true, &title_db, &ui_tx);
                ui_tx
                    .send(UiMsg::Ready {
                        titles: title_db.titles_sorted_vec(),
                        sync_states: state_db.states_hashmap(),
                        qty_auto: state_db.qty_auto(),
                    })
                    .ok();
            }
            Ok(TaskMsg::Toggle(title_id)) => {
                state_db.toggle_auto_sync_for_title(title_id)?;
                ui_tx
                    .send(UiMsg::Ready {
                        titles: title_db.titles_sorted_vec(),
                        sync_states: state_db.states_hashmap(),
                        qty_auto: state_db.qty_auto(),
                    })
                    .ok();
            }
            Ok(TaskMsg::SyncAuto) => {
                let started_at = Instant::now();

                let mut ordered_states = state_db.states_mut().collect_vec();
                ordered_states.sort_by(|l, r| l.sync_item.cmp(&r.sync_item));

                match sync::run(
                    ordered_states.into_iter().filter(|s| s.auto_enabled),
                    &title_db,
                    &shutdown_rx,
                    ui_tx.clone(),
                    modal_tx.clone(),
                    &mut install_history_db,
                ) {
                    Ok(_) => {
                        ui_tx
                            .send(UiMsg::SyncDone {
                                result: "Sync completed".into(),
                                message: format!(
                                    "in {} seconds",
                                    Instant::now().duration_since(started_at).as_secs()
                                ),
                            })
                            .ok();
                    }
                    Err(e) => {
                        ui_tx
                            .send(UiMsg::SyncDone {
                                result: "Sync failed".into(),
                                message: format!(
                                    "at {}",
                                    chrono::Utc::now().format("%Y-%m-%d %H:%M")
                                ),
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
                    &title_db,
                    &shutdown_rx,
                    ui_tx.clone(),
                    modal_tx.clone(),
                    &mut install_history_db,
                ) {
                    Ok(qty) => {
                        ui_tx
                            .send(UiMsg::SyncDone {
                                result: "Sync completed".into(),
                                message: format!(
                                    "for {qty} sync {}",
                                    match qty {
                                        1 => "item",
                                        _ => "items",
                                    }
                                ),
                            })
                            .ok();
                    }
                    Err(e) => {
                        ui_tx
                            .send(UiMsg::SyncDone {
                                result: "Sync failed".into(),
                                message: format!(
                                    "at {}",
                                    chrono::Utc::now().format("%Y-%m-%d %H:%M")
                                ),
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
                    .send(UiMsg::Ready {
                        titles: title_db.titles_sorted_vec(),
                        sync_states: state_db.states_hashmap(),
                        qty_auto: state_db.qty_auto(),
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
