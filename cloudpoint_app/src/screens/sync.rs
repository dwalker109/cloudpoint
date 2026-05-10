use super::*;
use crate::app::TaskMsg;
use std::sync::mpsc::Sender;

pub struct SyncScreen {
    task_tx: Sender<TaskMsg>,
    sync_running: bool,
    upper_1: String,
    upper_2: String,
    lower_1: String,
}

impl SyncScreen {
    pub fn new(task_tx: Sender<TaskMsg>) -> Self {
        task_tx.send(TaskMsg::ReadySync).ok();

        Self {
            task_tx,
            sync_running: false,
            upper_1: String::with_capacity(256),
            upper_2: String::with_capacity(256),
            lower_1: String::with_capacity(256),
        }
    }
}

impl Screen for SyncScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        ctx.rect(0.0, 0.0, TOP_W, TOP_H, WHITE);
        ctx.rect(0.0, 0.0, TOP_W, 32.0, ACCENT);
        ctx.text_centered(0.0, 6.0, TOP_W, 0.7, WHITE, "Cloudpoint Sync");
        ctx.text_centered(0.0, 100.0, TOP_W, 0.6, BLACK, &self.upper_1);
        ctx.text_centered(0.0, 120.0, TOP_W, 0.6, BLACK, &self.upper_2);
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        ctx.rect(0.0, 0.0, BOT_W, BOT_H, ACCENT);
        ctx.text_centered(0.0, 110.0, BOT_W, 0.6, BLACK, &self.lower_1);
    }
}

impl BaseScreen for SyncScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Sync
    }

    fn handle_msg(&mut self, msg: &UiMsg) {
        match msg {
            UiMsg::SyncReady { total_states } => {
                self.upper_1 = "Ready to sync".into();
                self.upper_2 = format!("{total_states} saves available");
                self.lower_1 = "Press (A) to sync now".into()
            }
            UiMsg::SyncProgress {
                title_short,
                message,
            } => {
                self.upper_1 = title_short.clone();
                self.upper_2 = message.clone();
            }
            UiMsg::SyncDone => {
                self.sync_running = false;
                self.upper_1 = "Last sync completed at".into();
                self.upper_2 = chrono::Utc::now().to_rfc2822();
                self.lower_1 = "Press (A) to start a new sync".into()
            }
            _ => {}
        }
    }

    fn handle_input(&mut self, keys: &KeyPad) -> ScreenCommand {
        if !self.sync_running && keys.contains(KeyPad::A) {
            self.sync_running = true;
            self.lower_1 = "...".into();
            self.task_tx.send(TaskMsg::StartSync).ok();
            ScreenCommand::Noop
        } else if keys.intersects(KeyPad::L | KeyPad::R) {
            ScreenCommand::SwitchTo(ScreenId::Games)
        } else {
            ScreenCommand::Noop
        }
    }
}
