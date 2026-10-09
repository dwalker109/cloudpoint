use super::*;
use crate::screens::shared::{dialog_lower, dialog_upper, modal_spinner};

enum Status {
    Running,
    Delayed,
    Failed(String),
}

pub struct ConnectModalScreen {
    status: Status,
}

impl ConnectModalScreen {
    pub fn new() -> Self {
        Self {
            status: Status::Running,
        }
    }
}

impl Screen for ConnectModalScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        dialog_upper(ctx);

        let message_1 = match self.status {
            Status::Running => "Connecting",
            Status::Delayed => "Still connecting",
            Status::Failed(_) => "Failed to connect",
        };

        let message_2 = match &self.status {
            Status::Running => "Please wait",
            Status::Delayed => "This is taking longer than expected",
            Status::Failed(msg) => &msg,
        };

        ctx.text_centered(0.0, 105.0, TOP_W, 0.6, BLACK, message_1);
        ctx.text_centered(0.0, 125.0, TOP_W, 0.5, DARK_GREY, message_2);
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        dialog_lower(ctx);

        match self.status {
            Status::Running | Status::Delayed => {
                modal_spinner(ctx, BOT_W / 2.0 - 15.0, BOT_H / 2.0 - 15.0, 1.5, ACCENT);
            }
            Status::Failed(..) => {
                ctx.text_centered(0.0, BOT_H / 2.0 - 10.0, BOT_W, 1.0, ACCENT, "\u{E009}");
            }
        }

        if matches!(self.status, Status::Delayed | Status::Failed(..)) {
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
}

impl ModalScreen for ConnectModalScreen {
    fn handle_msg(&mut self, msg: &UiMsg) -> ScreenCommand {
        match msg {
            UiMsg::ConnectDelayed => self.status = Status::Delayed,
            UiMsg::ConnectDone => {
                return ScreenCommand::CloseModal;
            }
            UiMsg::ConnectFailed { reason } => {
                self.status = Status::Failed(reason.clone());
            }
            _ => {}
        }

        ScreenCommand::Noop
    }

    fn handle_input(&mut self, _keys_down: &KeyPad, _keys_held: &KeyPad) -> ScreenCommand {
        ScreenCommand::Noop
    }
}
