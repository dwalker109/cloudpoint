use super::*;
use crate::{
    app::TaskMsg,
    config::{APP_VER, USER_KEY},
};
use std::sync::mpsc::Sender;

pub struct SyncScreen {
    task_tx: Sender<TaskMsg>,
    status_text: String,
}

impl SyncScreen {
    pub fn new(task_tx: Sender<TaskMsg>) -> Self {
        Self {
            task_tx,
            status_text: String::with_capacity(256),
        }
    }
}

impl Screen for SyncScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        super::shared::header(ctx, self.id());

        ctx.text_centered(0.0, 100.0, TOP_W, 1.0, ACCENT, "Ready");
        ctx.text_centered(0.0, 140.0, TOP_W, 0.7, BLACK, &self.status_text);
        ctx.text_centered(
            0.0,
            TOP_H - 35.0,
            TOP_W,
            0.4,
            GREY_TRANS,
            &format!("Ver {}", *APP_VER),
        );
        ctx.text_centered(
            0.0,
            TOP_H - 20.0,
            TOP_W,
            0.4,
            GREY_TRANS,
            &format!("User {}", *USER_KEY.as_hyphenated()),
        );
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        ctx.rect(0.0, 0.0, BOT_W, BOT_H, ACCENT);
        ctx.text_centered(0.0, 90.0, BOT_W, 0.9, WHITE, "\u{E000} Auto Sync");
        ctx.text_centered(0.0, 130.0, BOT_W, 0.9, WHITE, "\u{E002} Refresh");
    }
}

impl BaseScreen for SyncScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Sync
    }

    fn handle_msg(&mut self, msg: &UiMsg) -> ScreenCommand {
        match msg {
            UiMsg::RefreshDone {
                qty_sync_states,
                titles,
            } => {
                self.status_text = format!(
                    "{qty_sync_states} items enabled across {} titles",
                    titles.len()
                );
            }
            _ => {}
        }

        ScreenCommand::Noop
    }

    fn handle_input(&mut self, keys_down: &KeyPad, _keys_held: &KeyPad) -> ScreenCommand {
        if keys_down.contains(KeyPad::A) {
            self.task_tx.send(TaskMsg::SyncAuto).ok();
            return ScreenCommand::OpenModal(Box::new(SyncModalScreen::new()));
        } else if keys_down.contains(KeyPad::X) {
            self.task_tx.send(TaskMsg::Refresh).ok();
        } else if keys_down.contains(KeyPad::L) {
            return ScreenCommand::SwitchTo(ScreenId::Titles);
        } else if keys_down.contains(KeyPad::R) {
            return ScreenCommand::SwitchTo(ScreenId::Link);
        }

        ScreenCommand::Noop
    }
}
