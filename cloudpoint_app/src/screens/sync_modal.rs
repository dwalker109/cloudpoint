use super::*;

pub struct SyncModalScreen {
    task_running: bool,
    upper_1: String,
    upper_2: String,
}

impl SyncModalScreen {
    pub fn new() -> Self {
        Self {
            task_running: true,
            upper_1: String::new(),
            upper_2: String::new(),
        }
    }
}

impl Screen for SyncModalScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        ctx.rect(20.0, 20.0, TOP_W - 40.0, TOP_H - 40.0, WHITE);
        ctx.text_centered(0.0, 100.0, TOP_W, 0.6, BLACK, &self.upper_1);
        ctx.text_centered(0.0, 120.0, TOP_W, 0.6, BLACK, &self.upper_2);
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        ctx.rect(20.0, 20.0, BOT_W - 40.0, BOT_H - 40.0, WHITE);
        let text = if self.task_running {
            "Please do not touch \u{E078}"
        } else {
            "\u{E000} Continue"
        };
        ctx.text_centered(0.0, 110.0, BOT_W, 0.6, ACCENT, &text);
    }
}

impl ModalScreen for SyncModalScreen {
    fn handle_msg(&mut self, msg: &UiMsg) -> ScreenCommand {
        match msg {
            UiMsg::SyncProgress { label, message, .. } => {
                self.upper_1 = label.clone();
                self.upper_2 = message.clone();
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
