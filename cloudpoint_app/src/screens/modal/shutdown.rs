use crate::screens::shared::{dialog_lower, dialog_upper};

use super::*;

pub struct ShutdownModalScreen;

impl ShutdownModalScreen {
    pub fn new() -> Self {
        Self
    }
}

impl ModalScreen for ShutdownModalScreen {}

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
