use super::*;
use crate::app::TaskMsg;
use std::sync::mpsc::Sender;

pub struct SyncScreen {
    task_tx: Sender<TaskMsg>,
    sync_running: bool,
    upper_1: String,
    upper_2: String,
    lower_1: String,
    lower_2: String,
}

impl SyncScreen {
    pub fn new(task_tx: Sender<TaskMsg>) -> Self {
        Self {
            task_tx,
            sync_running: false,
            upper_1: String::with_capacity(256),
            upper_2: String::with_capacity(256),
            lower_1: "(A) to sync".into(),
            lower_2: "(X) to refresh".into(),
        }
    }
}

impl Screen for SyncScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        ctx.rect(0.0, 0.0, TOP_W, TOP_H, WHITE);
        ctx.rect(0.0, 0.0, TOP_W, 32.0, ACCENT);
        ctx.text_centered(0.0, 6.0, TOP_W, 0.7, WHITE, "Sync");
        ctx.text_centered(0.0, 116.0, TOP_W, 0.6, BLACK, &self.upper_1);
        ctx.text_centered(0.0, 136.0, TOP_W, 0.6, BLACK, &self.upper_2);
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        ctx.rect(0.0, 0.0, BOT_W, BOT_H, ACCENT);
        let colour = if self.sync_running { DARK_GREY } else { BLACK };
        ctx.text_centered(0.0, 100.0, BOT_W, 0.6, colour, &self.lower_1);
        ctx.text_centered(0.0, 120.0, BOT_W, 0.6, colour, &self.lower_2);
    }
}

impl BaseScreen for SyncScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Sync
    }

    fn handle_msg(&mut self, msg: &UiMsg) -> ScreenCommand {
        match msg {
            UiMsg::RefreshDone {
                qty_sync_states,
                titles,
            } => {
                self.upper_1 = "Ready to sync".into();
                self.upper_2 = format!(
                    "{qty_sync_states} auto sync items enabled across {} titles",
                    titles.len()
                );
            }
            UiMsg::SyncProgress { title_lbl, message } => {
                self.upper_1 = title_lbl.clone();
                self.upper_2 = message.clone();
            }
            UiMsg::SyncDone { result, message } => {
                self.sync_running = false;
                self.upper_1 = result.clone();
                self.upper_2 = message.clone();
            }
            _ => {}
        }

        ScreenCommand::Noop
    }

    fn handle_input(&mut self, keys_down: &KeyPad, _keys_held: &KeyPad) -> ScreenCommand {
        if !self.sync_running && keys_down.contains(KeyPad::A) {
            self.sync_running = true;
            self.task_tx.send(TaskMsg::SyncAuto).ok();
        } else if !self.sync_running && keys_down.contains(KeyPad::X) {
            self.task_tx.send(TaskMsg::Refresh).ok();
        } else if keys_down.intersects(KeyPad::L | KeyPad::R) {
            return ScreenCommand::SwitchTo(ScreenId::Titles);
        }

        ScreenCommand::Noop
    }
}
