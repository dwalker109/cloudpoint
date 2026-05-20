use super::*;
use crate::app::TaskMsg;
use std::sync::mpsc::Sender;

pub struct SyncScreen {
    task_tx: Sender<TaskMsg>,
    sync_running: bool,
    progress: usize,
    upper_1: String,
    upper_2: String,
}

impl SyncScreen {
    pub fn new(task_tx: Sender<TaskMsg>) -> Self {
        Self {
            task_tx,
            sync_running: false,
            progress: 0,
            upper_1: String::with_capacity(256),
            upper_2: String::with_capacity(256),
        }
    }
}

impl Screen for SyncScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        super::shared::header(ctx, self.id());

        ctx.text_centered(0.0, 116.0, TOP_W, 0.6, BLACK, &self.upper_1);
        ctx.text_centered(0.0, 136.0, TOP_W, 0.6, BLACK, &self.upper_2);
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        ctx.rect(0.0, 0.0, BOT_W, BOT_H, ACCENT);
        if self.sync_running {
            ctx.rect(40.0, 110.0, 240.0, 24.0, GREY_TRANS);
            ctx.rect(
                40.0,
                110.0,
                self.progress as f32 * 240.0 / 100.0,
                24.0,
                WHITE,
            );
            ctx.text_centered(
                0.0,
                210.0,
                BOT_W,
                0.6,
                WHITE,
                "Please do not touch \u{E078}",
            );
        } else {
            ctx.text_centered(0.0, 90.0, BOT_W, 0.9, WHITE, "\u{E000} Auto Sync");
            ctx.text_centered(0.0, 130.0, BOT_W, 0.9, WHITE, "\u{E002} Refresh");
        }
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
                self.upper_1 = "Ready to auto sync".into();
                self.upper_2 = format!(
                    "{qty_sync_states} items enabled across {} titles",
                    titles.len()
                );
            }
            UiMsg::SyncProgress {
                label: title_lbl,
                message,
                progress,
            } => {
                self.upper_1 = title_lbl.clone();
                self.upper_2 = message.clone();
                self.progress = *progress;
            }
            UiMsg::SyncDone { result, message } => {
                self.sync_running = false;
                self.progress = 0;
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
        } else if keys_down.contains(KeyPad::L) {
            return ScreenCommand::SwitchTo(ScreenId::Titles);
        } else if keys_down.contains(KeyPad::R) {
            return ScreenCommand::SwitchTo(ScreenId::Help);
        }

        ScreenCommand::Noop
    }
}
