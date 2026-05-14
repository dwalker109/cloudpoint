use crate::{
    ctr_gfx::Render,
    screens::{
        BaseScreen, ConflictModalScreen, ModalScreen, RefreshModalScreen, ScreenCommand, ScreenId,
        SyncScreen, TitlesScreen,
    },
    services::CtrServices,
    setup,
};
use anyhow::Result;
use ctru::services::hid::KeyPad;
use std::{
    collections::HashMap,
    sync::mpsc::{Receiver, Sender, channel},
};
pub use worker::*;

mod worker;

pub struct App {
    screens: HashMap<ScreenId, Box<dyn BaseScreen>>,
    active_screen: ScreenId,
    modal_stack: Vec<Box<dyn ModalScreen>>,
    _task_tx: Sender<TaskMsg>,
    ui_rx: Receiver<UiMsg>,
    alert_rx: Receiver<ModalMsg>,
}

impl App {
    pub fn run(mut services: CtrServices) -> Result<()> {
        let (task_tx, task_rx) = channel::<TaskMsg>();
        let (ui_tx, ui_rx) = channel::<UiMsg>();
        let (modal_tx, modal_rx) = channel::<ModalMsg>();

        let handle = setup::start_worker(task_rx, ui_tx, modal_tx)?;

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
            alert_rx: modal_rx,
        };

        let mut render = Render::new();
        let mut cmd_buffer = Vec::with_capacity(8);

        while services.apt.main_loop() {
            services.hid.scan_input();
            let keys_down = services.hid.keys_down();
            let keys_held = services.hid.keys_held();

            if keys_down.contains(KeyPad::START) {
                break;
            }

            if let Ok(msg) = app.alert_rx.try_recv() {
                match msg {
                    ModalMsg::ResolveConflict {
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
                    ModalMsg::Refresh => {
                        app.modal_stack.push(Box::new(RefreshModalScreen::new()));
                    }
                }
            }

            if let Ok(msg) = app.ui_rx.try_recv() {
                for screen in app.screens.values_mut() {
                    cmd_buffer.push(screen.handle_msg(&msg));
                }

                for modal in app.modal_stack.iter_mut() {
                    cmd_buffer.push(modal.handle_msg(&msg));
                }
            }

            if let Some(modal) = app.modal_stack.last_mut() {
                cmd_buffer.push(modal.handle_input(&keys_down, &keys_held));
            } else {
                let active_screen = app.screens.get_mut(&app.active_screen).unwrap();
                cmd_buffer.push(active_screen.handle_input(&keys_down, &keys_held));
            }

            for cmd in cmd_buffer.drain(..) {
                match cmd {
                    ScreenCommand::SwitchTo(id) => {
                        app.active_screen = id;
                    }
                    ScreenCommand::OpenModal(screen) => {
                        app.modal_stack.push(screen);
                    }
                    ScreenCommand::CloseModal => {
                        app.modal_stack.pop();
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
