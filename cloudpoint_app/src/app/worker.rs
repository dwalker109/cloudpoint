use super::*;
use crate::{
    config::AppPath,
    db::{StateDb, TitleDb},
    sync,
};
use cloudpoint_lib::http::CurlHttpClient;
use std::{
    rc::Rc,
    sync::mpsc::{Receiver, Sender},
};

pub fn worker_thread(task_rx: Receiver<TaskMsg>, ui_tx: Sender<UiMsg>, modal_tx: Sender<ModalMsg>) {
    let (mut state_db, mut title_db) = {
        if let (Ok(state_db), Ok(title_db)) =
            (StateDb::open(AppPath::Db), TitleDb::open(AppPath::Db))
        {
            (state_db, title_db)
        } else {
            modal_tx.send(ModalMsg::Refresh).ok();
            let state_db = StateDb::new(AppPath::Db, &ui_tx).expect("state db should be creatable");
            let title_db =
                TitleDb::new(AppPath::Db, &state_db, &ui_tx).expect("title db should be creatable");

            (state_db, title_db)
        }
    };

    ui_tx
        .send(UiMsg::RefreshDone {
            qty_sync_states: state_db.qty_auto(),
            titles: title_db.titles_sorted_vec(),
        })
        .ok();

    let client = Rc::new(CurlHttpClient::new().expect("curl client should be available"));

    loop {
        match task_rx.recv() {
            Ok(TaskMsg::Refresh) => {
                modal_tx.send(ModalMsg::Refresh).ok();
                state_db.refresh(true, &ui_tx).ok();
                title_db.refresh(&state_db, &ui_tx).ok();
                ui_tx
                    .send(UiMsg::RefreshDone {
                        qty_sync_states: state_db.qty_auto(),
                        titles: title_db.titles_sorted_vec(),
                    })
                    .ok();
            }
            Ok(TaskMsg::Toggle(title_id)) => {
                state_db.refresh_for_title_id(title_id, false).ok();
                state_db.toggle_for_title_id(title_id).ok();
                title_db.refresh_links(title_id, &state_db).ok();
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
                    ui_tx.clone(),
                    modal_tx.clone(),
                    &client,
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
                            .send(ModalMsg::Error {
                                label: "Error".into(),
                                message: e.to_string(),
                            })
                            .ok();
                    }
                };
            }
            Ok(TaskMsg::SyncTargeted(title_id)) => {
                state_db.refresh_for_title_id(title_id, false).ok();
                title_db.refresh_links(title_id, &state_db).ok();
                match sync::run(
                    state_db
                        .states_mut()
                        .filter(|s| s.via_title_ids.contains(&title_id)),
                    ui_tx.clone(),
                    modal_tx.clone(),
                    &client,
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
                            .send(ModalMsg::Error {
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
            Err(_) => return,
        }
    }
}
