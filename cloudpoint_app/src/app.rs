use crate::{
    gfx::Render,
    screens::{
        BaseScreen, ConflictModalScreen, ConnectModalScreen, ErrorModalScreen,
        LinkClientModalScreen, LinkHostModalScreen, LinkScreen, ModalScreen, RefreshModalScreen,
        ScreenCommand, ScreenId, ShutdownModalScreen, SyncScreen, TitlesScreen, UpdaterModalScreen,
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
    thread::JoinHandle,
};
pub use worker::worker_thread;

mod msg;
mod worker;

pub struct App {
    screens: HashMap<ScreenId, Box<dyn BaseScreen>>,
    active_screen: ScreenId,
    modal_stack: Vec<Box<dyn ModalScreen>>,
    task_tx: Sender<TaskMsg>,
    _shutdown_tx: Sender<()>,
    ui_rx: Receiver<UiMsg>,
    modal_rx: Receiver<OpenModalMsg>,
}

impl App {
    /// Set up comms channels, start the worker and enter the libctru main_loop.
    pub fn run() -> Result<()> {
        let apt = Apt::new()?;
        let mut hid = Hid::new()?;

        let (task_tx, task_rx) = channel::<TaskMsg>();
        let (shutdown_tx, shutdown_rx) = channel::<()>();
        let (ui_tx, ui_rx) = channel::<UiMsg>();
        let (modal_tx, modal_rx) = channel::<OpenModalMsg>();

        let handle = setup::start_worker(task_rx, shutdown_rx, ui_tx, modal_tx);

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
            task_tx,
            _shutdown_tx: shutdown_tx,
            ui_rx,
            modal_rx,
        };

        let mut render = Render::new();
        let mut cmd_buffer = Vec::with_capacity(8);

        log::debug!("entering main_loop");

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
                    OpenModalMsg::Connect => {
                        app.modal_stack.push(Box::new(ConnectModalScreen::new()));
                    }
                    OpenModalMsg::Refresh => {
                        app.modal_stack.push(Box::new(RefreshModalScreen::new()));
                    }
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
                    OpenModalMsg::SelfUpdate(client_update) => app.modal_stack.push(Box::new(
                        UpdaterModalScreen::new(app.task_tx.clone(), client_update),
                    )),
                }
            }

            render.frame(
                app.screens.get_mut(&app.active_screen).unwrap().as_ref(),
                app.modal_stack.last().map(|m| m.as_ref()),
            );
        }

        log::debug!("exited main_loop");

        Self::shutdown(app, handle);

        Ok(())
    }

    /// Controlled shutdown of the app once the libctru main_loop has exited.
    /// App is dropped, which leads to desirable channel closure when owned
    /// items drop too:
    ///
    /// - `modal_stack` (causing reply_tx channels to close, causing the blocked
    /// reply_rx channels to disconnect and exit)
    /// - `_task_tx` (allowing any running worker task to finish, but signals
    /// a disconnect on next poll of task_rx, exiting cleanly).
    ///
    /// All of this means that the handle will then join when asked, allowing
    /// a clean shutdown instead of just killing the worker at a potentially
    /// bad point (like halfway through a file write).
    fn shutdown(app: App, handle: JoinHandle<()>) {
        drop(app);
        handle.join().expect("worker thread should join on exit");
    }
}
