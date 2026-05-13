use super::*;
use crate::app::TaskMsg;
use std::sync::mpsc::Sender;

pub struct SyncScreen {
    task_tx: Sender<TaskMsg>,
    task_running: bool,
    upper_1: String,
    upper_2: String,
    lower_1: String,
    lower_2: String,
}

impl SyncScreen {
    pub fn new(task_tx: Sender<TaskMsg>) -> Self {
        task_tx.send(TaskMsg::SyncReady).ok();

        Self {
            task_tx,
            task_running: false,
            upper_1: String::with_capacity(256),
            upper_2: String::with_capacity(256),
            lower_1: "Press (A) to sync now".into(),
            lower_2: "Press (X) to autodiscover".into(),
        }
    }
}

impl Screen for SyncScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        ctx.rect(0.0, 0.0, TOP_W, TOP_H, WHITE);
        ctx.rect(0.0, 0.0, TOP_W, 32.0, ACCENT);
        ctx.text_centered(0.0, 6.0, TOP_W, 0.7, WHITE, "Sync");
        ctx.text_centered(0.0, 100.0, TOP_W, 0.6, BLACK, &self.upper_1);
        ctx.text_centered(0.0, 120.0, TOP_W, 0.6, BLACK, &self.upper_2);
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        ctx.rect(0.0, 0.0, BOT_W, BOT_H, ACCENT);
        let colour = if self.task_running { DARK_GREY } else { BLACK };
        ctx.text_centered(0.0, 100.0, BOT_W, 0.6, colour, &self.lower_1);
        ctx.text_centered(0.0, 120.0, BOT_W, 0.6, colour, &self.lower_2);
    }
}

impl BaseScreen for SyncScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Sync
    }

    fn handle_msg(&mut self, msg: &UiMsg) {
        match msg {
            UiMsg::SyncReady { total_states } => {
                self.task_running = false;
                self.upper_1 = "Ready to sync".into();
                self.upper_2 = format!("{total_states} saves available");
            }
            UiMsg::SyncProgress {
                title_short,
                message,
            } => {
                self.upper_1 = title_short.clone();
                self.upper_2 = message.clone();
            }
            UiMsg::SyncDone { result, message } => {
                self.task_running = false;
                self.upper_1 = result.clone();
                self.upper_2 = message.clone();
            }
            _ => {}
        }
    }

    fn handle_input(&mut self, keys_down: &KeyPad, _keys_held: &KeyPad) -> ScreenCommand {
        if !self.task_running && keys_down.contains(KeyPad::A) {
            self.task_running = true;
            self.task_tx.send(TaskMsg::SyncAllAuto).ok();
        } else if !self.task_running && keys_down.contains(KeyPad::X) {
            self.task_running = true;
            self.task_tx.send(TaskMsg::DiscoverAll).ok();
            self.task_tx.send(TaskMsg::TitleDbInvalidate).ok();
        } else if keys_down.intersects(KeyPad::L | KeyPad::R) {
            return ScreenCommand::SwitchTo(ScreenId::Titles);
        }

        ScreenCommand::Noop
    }
}
