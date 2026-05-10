use crate::{
    config::AppPath,
    ctr_gfx::Render,
    db::StateDb,
    screens::{
        BaseScreen, ConflictModalScreen, GamesScreen, ModalScreen, ScreenCommand, ScreenId,
        SyncScreen,
    },
    services, setup,
};
use anyhow::Result;
use ctru::services::hid::KeyPad;
pub use messaging::*;
use std::{
    collections::HashMap,
    sync::{
        Arc, RwLock,
        mpsc::{Receiver, Sender},
    },
    thread::JoinHandle,
};

mod messaging;

pub struct App {
    state_db: Arc<RwLock<StateDb>>,
    screens: HashMap<ScreenId, Box<dyn BaseScreen>>,
    active_screen: ScreenId,
    modal_stack: Vec<Box<dyn ModalScreen>>,
    task_tx: Sender<TaskMsg>,
    ui_rx: Receiver<UiMsg>,
    alert_rx: Receiver<AlertMsg>,
    _work_thread_handle: Option<JoinHandle<()>>,
}

impl App {
    pub fn run() -> Result<()> {
        let mut services = services::CtrServices::init()?;
        let state_db = Arc::new(RwLock::new(StateDb::open(AppPath::Db)?));
        let (handle, task_tx, ui_rx, alert_rx) = setup::start_worker(Arc::clone(&state_db))?;

        let mut app = App {
            state_db,
            screens: HashMap::from([
                (
                    ScreenId::Sync,
                    Box::new(SyncScreen::new(task_tx.clone())) as Box<dyn BaseScreen>,
                ),
                (
                    ScreenId::Games,
                    Box::new(GamesScreen::new(task_tx.clone())) as Box<dyn BaseScreen>,
                ),
            ]),
            active_screen: ScreenId::Sync,
            modal_stack: Vec::with_capacity(4),
            task_tx,
            ui_rx,
            alert_rx,
            _work_thread_handle: Some(handle),
        };

        let mut render = Render::new();

        while services.apt.main_loop() {
            services.hid.scan_input();
            let keys = services.hid.keys_down();

            if keys.contains(KeyPad::START) {
                app.task_tx.send(TaskMsg::Shutdown)?;
                break;
            }

            if let Ok(msg) = app.alert_rx.try_recv() {
                match msg {
                    AlertMsg::ResolveConflict {
                        title_short,
                        is_first_sync,
                        reply_tx,
                    } => {
                        app.modal_stack.push(Box::new(ConflictModalScreen::new(
                            title_short,
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
            }

            if let Some(modal) = app.modal_stack.last_mut() {
                match modal.handle_input(&keys) {
                    ScreenCommand::CloseModal => {
                        app.modal_stack.pop();
                    }
                    _ => {}
                }
            } else {
                let active_screen = app.screens.get_mut(&app.active_screen).unwrap();

                match active_screen.handle_input(&keys) {
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

        Ok(())
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self._work_thread_handle
            .take()
            .expect("handle should be initialised")
            .join()
            .expect("handle should be ready to join")
    }
}
