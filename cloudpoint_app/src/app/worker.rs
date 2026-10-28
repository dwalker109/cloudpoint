use super::*;
use crate::{
    config::RUNNING_VIA_HBL,
    db, link,
    net::{self, self_update, sync},
};
use anyhow::{Result, bail};
use itertools::Itertools;
use std::{
    sync::mpsc::{self, Receiver, Sender},
    time::Instant,
};

static COMMIT_MSG: &'static str = "should commit db to sd card";

pub fn worker_thread(
    task_rx: Receiver<TaskMsg>,
    shutdown_rx: Receiver<()>,
    ui_tx: Sender<UiMsg>,
    modal_tx: Sender<OpenModalMsg>,
) -> Result<()> {
    let (mut title_db, mut state_db, mut install_history_db) = db::load(&ui_tx, &modal_tx)?;
    let connect = net::connect::run(&shutdown_rx, &ui_tx)?;

    if let Some(client_update) = &connect.app_update
        && !*RUNNING_VIA_HBL
    {
        modal_tx
            .send(OpenModalMsg::SelfUpdate(client_update.clone()))
            .ok();
    } else {
        ui_tx.send(UiMsg::ConnectDone).ok();
    }

    ui_tx
        .send(UiMsg::Ready {
            titles: title_db.titles_sorted_vec(),
            sync_states: state_db.states_hashmap(),
            qty_auto: state_db.qty_auto(),
            connect: connect.clone(),
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
                        connect: connect.clone(),
                    })
                    .ok();
                title_db.commit().expect(COMMIT_MSG);
                state_db.commit().expect(COMMIT_MSG);
            }
            Ok(TaskMsg::Toggle(title_id)) => {
                state_db.toggle_auto_sync_for_title(title_id)?;
                ui_tx
                    .send(UiMsg::Ready {
                        titles: title_db.titles_sorted_vec(),
                        sync_states: state_db.states_hashmap(),
                        qty_auto: state_db.qty_auto(),
                        connect: connect.clone(),
                    })
                    .ok();
                state_db.commit().expect(COMMIT_MSG);
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
                install_history_db.commit().expect(COMMIT_MSG);
                state_db.commit().expect(COMMIT_MSG);
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
                install_history_db.commit().expect(COMMIT_MSG);
                state_db.commit().expect(COMMIT_MSG);
            }
            Ok(TaskMsg::LinkHost) => {
                if let Err(e) = link::host(&ui_tx, &modal_tx) {
                    log::error!("errored during user key share as host: {e}");
                    ui_tx
                        .send(UiMsg::LinkProgress {
                            state: link::LinkState::Failed,
                        })
                        .ok();
                }
            }
            Ok(TaskMsg::LinkClient) => {
                if let Err(e) = link::client(&ui_tx, &modal_tx) {
                    log::error!("errored during user key share as client: {e}");
                    ui_tx
                        .send(UiMsg::LinkProgress {
                            state: link::LinkState::Failed,
                        })
                        .ok();
                }
            }
            Ok(TaskMsg::UpdateStart(update)) => {
                let msg = match self_update::run(&update) {
                    Ok(_) => UiMsg::UpdateDone,
                    Err(e) => {
                        log::error!("self update failed: {e}");
                        UiMsg::UpdateFailed
                    }
                };
                ui_tx.send(msg).ok();
            }
            Ok(TaskMsg::UpdateSkip(update)) => {
                log::info!("update {} was skipped", update.version);
                ui_tx.send(UiMsg::ConnectDone).ok();
            }
            Err(e) if e == mpsc::RecvError => {
                log::info!("worker thread task channel shutting down");
                log::debug!("{e}");
                break;
            }
            Err(e) => {
                bail!("worker thread task channel error: {e} ")
            }
        }
    }

    Ok(())
}
