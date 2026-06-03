use cloudpoint_lib::utils::wrap;

use super::*;

pub struct ErrorModalScreen {
    upper_1: String,
    upper_2: String,
}

impl ErrorModalScreen {
    pub fn new(label: String, message: String) -> Self {
        Self {
            upper_1: label,
            upper_2: wrap(&message, 40),
        }
    }
}

impl Screen for ErrorModalScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        ctx.rect(20.0, 20.0, TOP_W - 40.0, TOP_H - 40.0, WHITE);
        ctx.text_centered(0.0, 40.0, TOP_W, 0.8, BLACK, &self.upper_1);
        ctx.text_centered(0.0, 80.0, TOP_W, 0.6, BLACK, &self.upper_2);
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        ctx.rect(20.0, 20.0, BOT_W - 40.0, BOT_H - 40.0, WHITE);
        ctx.text_centered(0.0, 110.0, BOT_W, 0.7, ACCENT, &"\u{E000} Continue");
    }
}

impl ModalScreen for ErrorModalScreen {
    fn handle_msg(&mut self, _msg: &UiMsg) -> ScreenCommand {
        ScreenCommand::Noop
    }

    fn handle_input(&mut self, keys_down: &KeyPad, _keys_held: &KeyPad) -> ScreenCommand {
        if keys_down == &KeyPad::A {
            return ScreenCommand::CloseModal;
        }

        ScreenCommand::Noop
    }
}
