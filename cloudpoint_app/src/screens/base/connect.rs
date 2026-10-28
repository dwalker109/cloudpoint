use super::*;
use crate::screens::shared::modal_spinner;

enum Status {
    Running,
    Delayed,
    Failed(String),
}

pub struct ConnectScreen {
    status: Status,
}

impl ConnectScreen {
    pub fn new() -> Self {
        Self {
            status: Status::Running,
        }
    }
}

impl BaseScreen for ConnectScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Connect
    }
}

impl Screen for ConnectScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        super::shared::header(ctx, self.id());

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

        ctx.text_centered(0.0, 100.0, TOP_W, 1.0, ACCENT, message_1);
        ctx.text_centered(0.0, 140.0, TOP_W, 0.7, BLACK, message_2);
    }

    fn draw_lower(&self, ctx: &DrawContext) {
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

    fn handle_msg(&mut self, msg: &UiMsg) -> ScreenCommand {
        match msg {
            UiMsg::ConnectDelayed => self.status = Status::Delayed,
            UiMsg::ConnectDone => {
                return ScreenCommand::SwitchTo(ScreenId::Sync);
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
