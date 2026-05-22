use std::sync::mpsc::Sender;

use crate::app::TaskMsg;

use super::*;
use itertools::Itertools;

pub struct LinkScreen {
    task_tx: Sender<TaskMsg>,
}

impl LinkScreen {
    pub fn new(task_tx: Sender<TaskMsg>) -> Self {
        Self { task_tx }
    }
}

impl Screen for LinkScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        super::shared::header(ctx, self.id());

        ctx.text(
            30.0,
            50.0,
            0.5,
            BLACK,
            &[
                "Link your consoles together to share your user key",
                "and sync all of them with the same saves and extdata.",
                "You can link as many as you like, one at a time.",
                "Visit this screen on two consoles simultaneously and",
                "press the appropriate button on each one to begin.",
            ]
            .iter()
            .join("\n"),
        );

        ctx.text_centered(
            40.0,
            150.0,
            TOP_W - 80.0,
            0.9,
            ACCENT,
            "\u{E002} Share key from this console",
        );

        ctx.text_centered(
            40.0,
            190.0,
            TOP_W - 80.0,
            0.9,
            ACCENT,
            "\u{E003} Receive key on this console",
        );
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        ctx.rect(0.0, 0.0, BOT_W, BOT_H, ACCENT);
        ctx.text_centered(
            0.0,
            14.0,
            BOT_W,
            0.48,
            WHITE,
            &[
                "Visit our GitHub for setup guides, help, and FAQs.",
                "Please also join our Discord! Let's be friends.",
            ]
            .iter()
            .join("\n"),
        );

        ctx.icon(ICON_HELP_QR, 70.0, 60.0, 0.75);
    }
}

impl BaseScreen for LinkScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Link
    }

    fn handle_msg(&mut self, _msg: &UiMsg) -> ScreenCommand {
        ScreenCommand::Noop
    }

    fn handle_input(&mut self, keys_down: &KeyPad, _keys_held: &KeyPad) -> ScreenCommand {
        if keys_down.contains(KeyPad::L) {
            return ScreenCommand::SwitchTo(ScreenId::Sync);
        } else if keys_down.contains(KeyPad::X) {
            self.task_tx.send(TaskMsg::LinkHost).ok();
        } else if keys_down.contains(KeyPad::Y) {
            self.task_tx.send(TaskMsg::LinkClient).ok();
        }

        ScreenCommand::Noop
    }
}
