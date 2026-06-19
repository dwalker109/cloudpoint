use super::*;
use crate::screens::shared::modal_spinner;

pub enum ConnectStatus {
    Running,
    Delayed,
    Failed(String),
}

pub struct ConnectModalScreen {
    status: ConnectStatus,
}

impl ConnectModalScreen {
    pub fn new() -> Self {
        Self {
            status: ConnectStatus::Running,
        }
    }
}

impl Screen for ConnectModalScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        ctx.rect(20.0, 20.0, TOP_W - 40.0, TOP_H - 40.0, WHITE);

        let message_1 = match self.status {
            ConnectStatus::Running => "Connecting",
            ConnectStatus::Delayed => "Still connecting",
            ConnectStatus::Failed(_) => "Failed to connect",
        };

        let message_2 = match &self.status {
            ConnectStatus::Running => "Please wait",
            ConnectStatus::Delayed => "This is taking longer than expected",
            ConnectStatus::Failed(msg) => &msg,
        };

        ctx.text_centered(0.0, 105.0, TOP_W, 0.6, BLACK, message_1);
        ctx.text_centered(0.0, 125.0, TOP_W, 0.5, BLACK, message_2);
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        ctx.rect(20.0, 20.0, BOT_W - 40.0, BOT_H - 40.0, WHITE);

        match self.status {
            ConnectStatus::Running | ConnectStatus::Delayed => {
                modal_spinner(ctx, BOT_W / 2.0 - 15.0, BOT_H / 2.0 - 15.0, 1.5, ACCENT);
            }
            ConnectStatus::Failed(..) => {
                ctx.text_centered(0.0, BOT_H / 2.0 - 10.0, BOT_W, 1.0, ACCENT, "\u{E009}");
            }
        }

        if matches!(
            self.status,
            ConnectStatus::Delayed | ConnectStatus::Failed(..)
        ) {
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
            UiMsg::ConnectDelayed => self.status = ConnectStatus::Delayed,
            UiMsg::ConnectDone => {
                return ScreenCommand::CloseModal;
            }
            UiMsg::ConnectFailed { reason } => {
                self.status = ConnectStatus::Failed(reason.clone());
            }
            _ => {}
        }

        ScreenCommand::Noop
    }

    fn handle_input(&mut self, _keys_down: &KeyPad, _keys_held: &KeyPad) -> ScreenCommand {
        ScreenCommand::Noop
    }
}
