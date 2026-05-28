use crate::{
    ctr_gfx::Render,
    screens::{
        BaseScreen, ConflictModalScreen, ErrorModalScreen, LinkClientModalScreen,
        LinkHostModalScreen, LinkScreen, ModalScreen, RefreshModalScreen, ScreenCommand, ScreenId,
        ShutdownModalScreen, SyncScreen, TitlesScreen,
    },
    setup,
};
use anyhow::Result;
use ctru::prelude::*;
use ctru::services::apt::Chainloader;
pub use msg::*;
use std::{
    collections::HashMap,
    sync::mpsc::{Receiver, Sender, channel},
};
pub use worker::worker_thread;

mod msg;
mod worker;

pub struct App {
    screens: HashMap<ScreenId, Box<dyn BaseScreen>>,
    active_screen: ScreenId,
    modal_stack: Vec<Box<dyn ModalScreen>>,
    _task_tx: Sender<TaskMsg>,
    _shutdown_tx: Sender<()>,
    ui_rx: Receiver<UiMsg>,
    modal_rx: Receiver<OpenModalMsg>,
}

impl App {
    pub fn run() -> Result<()> {
        let apt = Apt::new()?;
        let mut hid = Hid::new()?;

        let (task_tx, task_rx) = channel::<TaskMsg>();
        let (shutdown_tx, shutdown_rx) = channel::<()>();
        let (ui_tx, ui_rx) = channel::<UiMsg>();
        let (modal_tx, modal_rx) = channel::<OpenModalMsg>();

        let handle = setup::start_worker(task_rx, shutdown_rx, ui_tx, modal_tx)?;

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
                (
                    ScreenId::Link,
                    Box::new(LinkScreen::new(task_tx.clone())) as Box<dyn BaseScreen>,
                ),
            ]),
            active_screen: ScreenId::Sync,
            modal_stack: Vec::with_capacity(4),
            _task_tx: task_tx,
            _shutdown_tx: shutdown_tx,
            ui_rx,
            modal_rx,
        };

        let mut render = Render::new();
        let mut cmd_buffer = Vec::with_capacity(8);

        log::info!("entering main_loop");

        'main: while apt.main_loop() {
            hid.scan_input();
            let keys_down = hid.keys_down();
            let keys_held = hid.keys_held();

            if keys_down.contains(KeyPad::START) {
                app.modal_stack.push(Box::new(ShutdownModalScreen::new()));
                render.frame(
                    app.screens.get_mut(&app.active_screen).unwrap().as_ref(),
                    app.modal_stack.last().map(|m| m.as_ref()),
                );

                break;
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
                    ScreenCommand::RestartApp => {
                        Chainloader::new(&apt).set_to_self();
                        break 'main;
                    }
                    _ => {}
                }
            }

            if let Ok(msg) = app.modal_rx.try_recv() {
                match msg {
                    OpenModalMsg::ResolveConflict {
                        title_label,
                        title_local_time,
                        title_remote_time,
                        reply_tx,
                    } => {
                        app.modal_stack.push(Box::new(ConflictModalScreen::new(
                            title_label,
                            title_local_time,
                            title_remote_time,
                            reply_tx,
                        )));
                    }
                    OpenModalMsg::Refresh => {
                        app.modal_stack.push(Box::new(RefreshModalScreen::new()));
                    }
                    OpenModalMsg::Error { label, message } => {
                        app.modal_stack
                            .push(Box::new(ErrorModalScreen::new(label, message)));
                    }
                    OpenModalMsg::LinkHost { quit_tx } => app
                        .modal_stack
                        .push(Box::new(LinkHostModalScreen::new(quit_tx))),
                    OpenModalMsg::LinkClient { fc, quit_tx } => app
                        .modal_stack
                        .push(Box::new(LinkClientModalScreen::new(fc, quit_tx))),
                }
            }

            render.frame(
                app.screens.get_mut(&app.active_screen).unwrap().as_ref(),
                app.modal_stack.last().map(|m| m.as_ref()),
            );
        }

        log::info!("exited main_loop, shutting down");

        // Dropping the app is important since it:
        // * drops modals (causing reply_tx channels to close, causing the blocked reply_rx channels to error and exit)
        // * drops task_tx (allowing running worker task to finish, but causes error the next time task_rx is polled, exiting cleanly)
        //
        // All of this means that the handle will join when asked, allowing a clean shutdown instead of just killing the
        // worker at a potentially bad point (like halfway through a file write).
        log::debug!("about to drop app and await worker exit");
        drop(app);
        match handle.join() {
            Ok(_) => log::debug!("app exited with clean worker join"),
            Err(_) => log::warn!("app exited and could not join worker, this is not expected"),
        }

        Ok(())
    }
}
