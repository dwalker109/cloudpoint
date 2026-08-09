use crate::screens::shared::{dialog_lower, dialog_upper};

use super::*;

pub struct ShutdownModalScreen;

impl ShutdownModalScreen {
    pub fn new() -> Self {
        Self
    }
}

impl Screen for ShutdownModalScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        dialog_upper(ctx);

        ctx.text_centered(0.0, 110.0, TOP_W, 0.6, BLACK, "Shutting down");
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        dialog_lower(ctx);

        ctx.text_centered(
            0.0,
            110.0,
            BOT_W,
            0.7,
            ACCENT,
            "Please do not touch \u{E078}",
        );
    }
}

impl ModalScreen for ShutdownModalScreen {
    fn handle_msg(&mut self, _msg: &UiMsg) -> ScreenCommand {
        ScreenCommand::Noop
    }

    fn handle_input(&mut self, _keys_down: &KeyPad, _keys_held: &KeyPad) -> ScreenCommand {
        ScreenCommand::Noop
    }
}
