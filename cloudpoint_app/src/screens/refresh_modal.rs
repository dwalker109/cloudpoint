use super::*;
use crate::screens::shared::{dialog_lower, dialog_upper, modal_spinner};

pub struct RefreshModalScreen {
    message: String,
    progress: usize,
}

impl RefreshModalScreen {
    pub fn new() -> Self {
        Self {
            message: String::new(),
            progress: 0,
        }
    }
}

impl Screen for RefreshModalScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        dialog_upper(ctx);

        ctx.text_centered(0.0, 110.0, TOP_W, 0.6, BLACK, &self.message);
        modal_spinner(ctx, TOP_W - 60.0, 30.0, 1.2, ACCENT);
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        dialog_lower(ctx);

        ctx.rounded_rect(40.0, 110.0, 240.0, 24.0, ROUND_RAD_SM, GREY);
        ctx.rounded_rect(
            40.0,
            110.0,
            self.progress as f32 * 240.0 / 100.0,
            24.0,
            ROUND_RAD_SM,
            ACCENT,
        );
    }
}

impl ModalScreen for RefreshModalScreen {
    fn handle_msg(&mut self, msg: &UiMsg) -> ScreenCommand {
        match msg {
            UiMsg::RefreshProgress { message, progress } => {
                self.message = message.clone();
                self.progress = *progress;
            }
            UiMsg::RefreshDone { .. } => {
                return ScreenCommand::CloseModal;
            }
            _ => {}
        }

        ScreenCommand::Noop
    }

    fn handle_input(&mut self, _keys_down: &KeyPad, _keys_held: &KeyPad) -> ScreenCommand {
        ScreenCommand::Noop
    }
}
