use super::*;
use crate::{
    app::TaskMsg,
    screens::shared::{dialog_lower, dialog_upper, modal_spinner},
};
use cloudpoint_lib::preflight::ClientUpdate;
use std::sync::mpsc::Sender;

#[derive(PartialEq, Eq)]
enum Status {
    Waiting,
    Updating,
    Done,
    Failed,
}

pub struct UpdaterModalScreen {
    task_tx: Sender<TaskMsg>,
    update: ClientUpdate,
    status: Status,
}

impl UpdaterModalScreen {
    pub fn new(task_tx: Sender<TaskMsg>, update: ClientUpdate) -> Self {
        Self {
            task_tx,
            update,
            status: Status::Waiting,
        }
    }
}

impl Screen for UpdaterModalScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        dialog_upper(ctx);

        let (message_1, message_2) = match self.status {
            Status::Waiting => (
                "A new app version is available to install",
                match self.update.required {
                    true => "You must install this update to continue",
                    false => "You may skip this update if you want to",
                },
            ),
            Status::Updating => (
                "Cloudpoint update is being installed",
                "This could take a couple of minutes",
            ),
            Status::Done => ("Cloudpoint update is complete", "Ready to launch"),
            Status::Failed => (
                "Cloudpoint update could not be installed",
                "Please exit and try again",
            ),
        };

        ctx.text_centered(0.0, 105.0, TOP_W, 0.6, BLACK, message_1);
        ctx.text_centered(0.0, 125.0, TOP_W, 0.5, DARK_GREY, message_2);
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        dialog_lower(ctx);

        const MID: f32 = BOT_H / 2.0;
        let msg = |y, m| ctx.text_centered(0.0, y, BOT_W, 0.7, ACCENT, m);

        match self.status {
            Status::Waiting if !self.update.required => {
                msg(MID - 20.0, "\u{E000} Install");
                msg(MID + 10.0, "\u{E001} Skip");
            }
            Status::Waiting => {
                msg(MID - 20.0, "\u{E000} Install");
                msg(MID + 10.0, "\u{E044} then \u{E002} to exit");
            }
            Status::Updating => {
                modal_spinner(ctx, BOT_W / 2.0 - 15.0, BOT_H / 2.0 - 15.0, 1.5, ACCENT);
                ctx.text_centered(
                    0.0,
                    190.0,
                    BOT_W,
                    0.6,
                    ACCENT,
                    "Please do not touch \u{E078}",
                );
            }
            Status::Done => {
                msg(MID - 10.0, "\u{E000} Restart");
            }
            Status::Failed => {
                msg(MID - 10.0, "\u{E044} then \u{E002} to exit");
            }
        }
    }
}

impl ModalScreen for UpdaterModalScreen {
    fn handle_msg(&mut self, msg: &UiMsg) -> ScreenCommand {
        match msg {
            UiMsg::UpdateDone => self.status = Status::Done,
            UiMsg::UpdateFailed => self.status = Status::Failed,
            _ => {}
        }

        ScreenCommand::Noop
    }

    fn handle_input(&mut self, keys_down: &KeyPad, _keys_held: &KeyPad) -> ScreenCommand {
        match self.status {
            Status::Waiting => {
                if keys_down.contains(KeyPad::A) {
                    self.task_tx
                        .send(TaskMsg::UpdateStart(self.update.clone()))
                        .ok();
                    self.status = Status::Updating;
                } else if keys_down.contains(KeyPad::B) && !self.update.required {
                    self.task_tx
                        .send(TaskMsg::UpdateSkip(self.update.clone()))
                        .ok();

                    return ScreenCommand::CloseModal;
                }
            }
            Status::Done => {
                if keys_down.contains(KeyPad::A) {
                    return ScreenCommand::RestartApp;
                }
            }
            _ => {}
        }

        ScreenCommand::Noop
    }
}
