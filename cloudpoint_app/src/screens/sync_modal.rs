use super::*;
use crate::screens::shared::modal_spinner;

pub struct SyncModalScreen {
    task_running: bool,
    progress: usize,
    upper_1: String,
    upper_2: String,
}

impl SyncModalScreen {
    pub fn new() -> Self {
        Self {
            task_running: true,
            progress: 0,
            upper_1: String::new(),
            upper_2: String::new(),
        }
    }
}

impl Screen for SyncModalScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        ctx.rect(20.0, 20.0, TOP_W - 40.0, TOP_H - 40.0, WHITE);
        ctx.text_centered(0.0, 105.0, TOP_W, 0.6, BLACK, &self.upper_1);
        ctx.text_centered(0.0, 125.0, TOP_W, 0.5, BLACK, &self.upper_2);

        if self.task_running {
            modal_spinner(ctx);
        }
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        ctx.rect(20.0, 20.0, BOT_W - 40.0, BOT_H - 40.0, WHITE);
        if self.task_running {
            ctx.rect(40.0, 110.0, 240.0, 20.0, ACCENT_TRANS);
            ctx.rect(
                40.0,
                110.0,
                self.progress as f32 * 240.0 / 100.0,
                20.0,
                ACCENT,
            );
            ctx.text_centered(
                0.0,
                190.0,
                BOT_W,
                0.6,
                ACCENT,
                "Please do not touch \u{E078}",
            );
        } else {
            ctx.text_centered(0.0, 110.0, BOT_W, 0.6, ACCENT, "\u{E000} Continue");
        }
    }
}

impl ModalScreen for SyncModalScreen {
    fn handle_msg(&mut self, msg: &UiMsg) -> ScreenCommand {
        match msg {
            UiMsg::SyncProgress {
                label,
                message,
                progress,
            } => {
                self.upper_1 = label.clone();
                self.upper_2 = message.clone();
                self.progress = *progress;
            }
            UiMsg::SyncDone { result, message } => {
                self.task_running = false;
                self.upper_1 = result.clone();
                self.upper_2 = message.clone();
            }
            _ => {}
        }

        ScreenCommand::Noop
    }

    fn handle_input(&mut self, keys_down: &KeyPad, _keys_held: &KeyPad) -> ScreenCommand {
        if !self.task_running && keys_down.contains(KeyPad::A) {
            return ScreenCommand::CloseModal;
        }

        ScreenCommand::Noop
    }
}
