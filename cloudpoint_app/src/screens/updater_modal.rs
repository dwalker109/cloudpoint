use super::*;
use crate::screens::shared::{dialog_lower, dialog_upper, modal_spinner};
use cloudpoint_lib::preflight::ClientUpdate;

pub struct UpdaterModalScreen {
    update: ClientUpdate,
}

impl UpdaterModalScreen {
    pub fn new(update: ClientUpdate) -> Self {
        Self { update }
    }
}

impl Screen for UpdaterModalScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        dialog_upper(ctx);

        let message_1 = format!(
            "Application version {} is available to install",
            self.update.version
        );
        let message_2 = match self.update.required {
            true => "You must install this update to continue",
            false => "You may skip this update if you want to",
        };

        ctx.text_centered(0.0, 105.0, TOP_W, 0.6, BLACK, &message_1);
        ctx.text_centered(0.0, 125.0, TOP_W, 0.5, DARK_GREY, message_2);
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        dialog_lower(ctx);

        // match self.status {
        //     ConnectStatus::Running | ConnectStatus::Delayed => {
        //         modal_spinner(ctx, BOT_W / 2.0 - 15.0, BOT_H / 2.0 - 15.0, 1.5, ACCENT);
        //     }
        //     ConnectStatus::Failed(..) => {
        //         ctx.text_centered(0.0, BOT_H / 2.0 - 10.0, BOT_W, 1.0, ACCENT, "\u{E009}");
        //     }
        // }

        ctx.text_centered(
            0.0,
            185.0,
            BOT_W,
            0.7,
            ACCENT,
            "\u{E044} then \u{E002} to exit",
        );
    }
}

impl ModalScreen for UpdaterModalScreen {
    fn handle_msg(&mut self, msg: &UiMsg) -> ScreenCommand {
        ScreenCommand::Noop
    }

    fn handle_input(&mut self, _keys_down: &KeyPad, _keys_held: &KeyPad) -> ScreenCommand {
        ScreenCommand::Noop
    }
}
