use super::*;

pub struct ShutdownModalScreen;

impl ShutdownModalScreen {
    pub fn new() -> Self {
        Self
    }
}

impl Screen for ShutdownModalScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        ctx.rect(20.0, 20.0, TOP_W - 40.0, TOP_H - 40.0, WHITE);
        ctx.text_centered(0.0, 110.0, TOP_W, 0.6, BLACK, "Shutting down");
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        ctx.rect(20.0, 20.0, BOT_W - 40.0, BOT_H - 40.0, WHITE);
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
