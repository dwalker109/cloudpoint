use super::*;
use crate::sync::ConflictWinner;
use std::sync::oneshot;

pub struct ConflictModalScreen {
    title_short: String,
    is_first_sync: bool,
    reply_tx: Option<oneshot::Sender<ConflictWinner>>,
}

impl ConflictModalScreen {
    pub fn new(
        title_short: String,
        is_first_sync: bool,
        reply_tx: oneshot::Sender<ConflictWinner>,
    ) -> Self {
        Self {
            title_short,
            is_first_sync,
            reply_tx: Some(reply_tx),
        }
    }
}

impl Screen for ConflictModalScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        ctx.rect(20.0, 20.0, TOP_W - 40.0, TOP_H - 40.0, WHITE);
        ctx.text_centered(20.0, 20.0, TOP_W - 40.0, 0.7, WHITE, &self.title_short);
        ctx.text_centered(
            30.0,
            30.0,
            TOP_W - 60.0,
            0.5,
            BLACK,
            match self.is_first_sync {
                true => "There is already data on the server for this title",
                false => {
                    "The data for this title has changed on this console as well as the server"
                }
            },
        );
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        ctx.rect(0.0, 0.0, BOT_W, BOT_H, ACCENT);
        ctx.button(
            40.0,
            60.0,
            BOT_W - 120.0,
            32.0,
            WHITE,
            ACCENT,
            "Keypad UP to keep and upload the data from this console",
            0.3,
        );
        ctx.button(
            40.0,
            120.0,
            BOT_W - 120.0,
            32.0,
            WHITE,
            ACCENT,
            "Keypad DOWN to download and replace the data on this console",
            0.3,
        );
        ctx.button(
            40.0,
            180.0,
            BOT_W - 120.0,
            32.0,
            WHITE,
            ACCENT,
            "Keypad LEFT or RIGHT to decide later",
            0.3,
        );
    }
}

impl ModalScreen for ConflictModalScreen {
    fn handle_input(&mut self, keys: &KeyPad) -> ScreenCommand {
        if keys.contains(KeyPad::UP) {
            // temporary: send Undecided just to see if this branch is reachable
            if let Some(tx) = self.reply_tx.take() {
                tx.send(ConflictWinner::Undecided).ok();
            }
            return ScreenCommand::CloseModal;
        }

        let winner = match keys {
            k if k.contains(KeyPad::DPAD_UP) => ConflictWinner::Local,
            k if k.contains(KeyPad::DPAD_DOWN) => ConflictWinner::Remote,
            k if k.intersects(KeyPad::DPAD_LEFT | KeyPad::DPAD_RIGHT) => ConflictWinner::Undecided,
            _ => return ScreenCommand::Noop,
        };

        self.reply_tx.take().unwrap().send(winner).ok();
        ScreenCommand::CloseModal
    }
}
