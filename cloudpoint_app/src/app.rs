use crate::{
    config::AppPath,
    ctr_gfx::Render,
    db::StateDb,
    screens::{
        BaseScreen, ConflictModalScreen, ModalScreen, ScreenCommand, ScreenId, SyncScreen,
        TitlesScreen,
    },
    services::CtrServices,
    setup,
    sync::ConflictWinner,
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use ctru::services::hid::KeyPad;
use std::{
    collections::HashMap,
    sync::{
        Arc, RwLock,
        mpsc::{Receiver, Sender, channel},
        oneshot,
    },
};
pub use worker::*;

mod worker;

pub struct App {
    screens: HashMap<ScreenId, Box<dyn BaseScreen>>,
    active_screen: ScreenId,
    modal_stack: Vec<Box<dyn ModalScreen>>,
    _task_tx: Sender<TaskMsg>,
    ui_rx: Receiver<UiMsg>,
    alert_rx: Receiver<AlertMsg>,
}

impl App {
    pub fn run(mut services: CtrServices) -> Result<()> {
        let (task_tx, task_rx) = channel::<TaskMsg>();
        let (ui_tx, ui_rx) = channel::<UiMsg>();
        let (alert_tx, alert_rx) = channel::<AlertMsg>();

        let handle = setup::start_worker(task_rx, ui_tx, alert_tx)?;

        let mut app = App {
            screens: HashMap::from([
                (
                    ScreenId::Sync,
                    Box::new(SyncScreen::new(task_tx.clone())) as Box<dyn BaseScreen>,
                ),
                (
                    ScreenId::Titles,
                    Box::new(TitlesScreen::new(task_tx.clone())) as Box<dyn BaseScreen>,
                ),
            ]),
            active_screen: ScreenId::Sync,
            modal_stack: Vec::with_capacity(4),
            _task_tx: task_tx,
            ui_rx,
            alert_rx,
        };

        let mut render = Render::new();

        while services.apt.main_loop() {
            services.hid.scan_input();
            let keys_down = services.hid.keys_down();
            let keys_held = services.hid.keys_held();

            if keys_down.contains(KeyPad::START) {
                break;
            }

            // if keys_down.contains(KeyPad::SELECT) {
            //     let (tx, rx) = oneshot::channel::<ConflictWinner>();
            //     app.modal_stack.push(Box::new(ConflictModalScreen::new(
            //         "Test1 (test)".into(),
            //         DateTime::<Utc>::from_timestamp(0, 0),
            //         true,
            //         tx,
            //     )));
            // }

            if let Ok(msg) = app.alert_rx.try_recv() {
                match msg {
                    AlertMsg::ResolveConflict {
                        title_label,
                        title_remote_time,
                        is_first_sync,
                        reply_tx,
                    } => {
                        app.modal_stack.push(Box::new(ConflictModalScreen::new(
                            title_label,
                            title_remote_time,
                            is_first_sync,
                            reply_tx,
                        )));
                    }
                }
            }

            if let Ok(msg) = app.ui_rx.try_recv() {
                for screen in app.screens.values_mut() {
                    screen.handle_msg(&msg);
                }

                for modal in app.modal_stack.iter_mut() {
                    modal.handle_msg(&msg);
                }
            }

            if let Some(modal) = app.modal_stack.last_mut() {
                let cmd = modal.handle_input(&keys_down, &keys_held);
                match cmd {
                    ScreenCommand::CloseModal => {
                        app.modal_stack.pop();
                    }
                    _ => {}
                }
            } else {
                let active_screen = app.screens.get_mut(&app.active_screen).unwrap();

                let cmd = active_screen.handle_input(&keys_down, &keys_held);
                match cmd {
                    ScreenCommand::SwitchTo(id) => {
                        app.active_screen = id;
                    }
                    ScreenCommand::OpenModal(screen) => {
                        app.modal_stack.push(screen);
                    }
                    _ => {}
                }
            }

            let active_screen = app.screens.get_mut(&app.active_screen).unwrap();

            render.frame(
                active_screen.as_ref(),
                app.modal_stack.last().map(|m| m.as_ref()),
            );
        }

        // Dropping the app is important since it:
        // * drops modals (causing reply_tx channels to close, causing the blocked reply_rx channels to error and exit)
        // * drops task_tx (allowing running worker task to finish, but causes error the next time task_rx is polled, exiting cleanly)
        //
        // All of this means that the handle will join when asked, allowing a clean shutdown instead of just killing the
        // worker at a potentially bad point (like halfway through a file write).
        drop(app);
        handle
            .join()
            .expect("worker thread should be joinable after shutdown");

        Ok(())
    }
}
