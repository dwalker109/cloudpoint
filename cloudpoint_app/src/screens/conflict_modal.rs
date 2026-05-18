use super::*;
use crate::sync::ConflictWinner;
use chrono::{DateTime, Utc};
use std::sync::oneshot;

pub struct ConflictModalScreen {
    title_label: String,
    title_local_time: Option<DateTime<Utc>>,
    title_remote_time: Option<DateTime<Utc>>,
    is_first_sync: bool,
    reply_tx: Option<oneshot::Sender<ConflictWinner>>,
    input_hold_up: u8,
    input_hold_down: u8,
}

impl ConflictModalScreen {
    pub fn new(
        title_label: String,
        title_local_time: Option<DateTime<Utc>>,
        title_remote_time: Option<DateTime<Utc>>,
        reply_tx: oneshot::Sender<ConflictWinner>,
    ) -> Self {
        Self {
            title_label,
            title_local_time,
            title_remote_time,
            is_first_sync: title_local_time.is_none(),
            reply_tx: Some(reply_tx),
            input_hold_up: 0,
            input_hold_down: 0,
        }
    }
}

impl Screen for ConflictModalScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        ctx.rect(20.0, 20.0, TOP_W - 40.0, TOP_H - 40.0, WHITE);
        ctx.text_centered(
            0.0,
            50.0,
            TOP_W,
            0.5,
            BLACK,
            match self.is_first_sync {
                true => "There is already data on the server for this title:",
                false => "This title has changed both here and on the server:",
            },
        );
        ctx.text_centered(0.0, 105.0, TOP_W, 0.7, BLACK, &self.title_label);

        ctx.text_centered(0.0, 160.0, TOP_W / 2.0, 0.55, DARK_GREY, "Last synced");
        ctx.text_centered(
            0.0,
            180.0,
            TOP_W / 2.0,
            0.5,
            BLACK,
            &self
                .title_local_time
                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "Never".into()),
        );

        ctx.text_centered(
            TOP_W / 2.0,
            160.0,
            TOP_W / 2.0,
            0.55,
            DARK_GREY,
            "Version on server",
        );
        ctx.text_centered(
            TOP_W / 2.0,
            180.0,
            TOP_W / 2.0,
            0.5,
            BLACK,
            &self
                .title_remote_time
                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "Unknown".into()),
        );
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        ctx.rect(20.0, 20.0, BOT_W - 40.0, BOT_H - 40.0, WHITE);
        ctx.text_centered(
            0.0,
            36.0,
            BOT_W,
            0.45,
            BLACK,
            "How would you like to proceed?",
        );
        ctx.button(
            40.0,
            64.0,
            BOT_W - 80.0,
            36.0,
            ACCENT,
            WHITE,
            "Hold UP: use the data from this console",
            0.4 + (self.input_hold_up as f32 / 2000.0),
        );
        ctx.button(
            40.0,
            114.0,
            BOT_W - 80.0,
            36.0,
            ACCENT,
            WHITE,
            "Hold DOWN: use the data from the server",
            0.4 + (self.input_hold_down as f32 / 2000.0),
        );
        ctx.button(
            40.0,
            164.0,
            BOT_W - 80.0,
            36.0,
            ACCENT,
            WHITE,
            "Press LEFT/RIGHT: skip for now",
            0.4,
        );
    }
}

impl ModalScreen for ConflictModalScreen {
    fn handle_msg(&mut self, _msg: &UiMsg) -> ScreenCommand {
        ScreenCommand::Noop
    }

    fn handle_input(&mut self, keys_down: &KeyPad, keys_held: &KeyPad) -> ScreenCommand {
        let mut winner = None;

        'check: {
            if keys_held == &KeyPad::DPAD_UP {
                self.input_hold_up += 1;
                if self.input_hold_up > 60 {
                    winner = Some(ConflictWinner::Local);
                }
                break 'check;
            } else {
                self.input_hold_up = 0;
            }

            if keys_held == &KeyPad::DPAD_DOWN {
                self.input_hold_down += 1;
                if self.input_hold_down > 60 {
                    winner = Some(ConflictWinner::Remote);
                }
                break 'check;
            } else {
                self.input_hold_down = 0;
            }

            if keys_down.intersects(KeyPad::DPAD_LEFT | KeyPad::DPAD_RIGHT) {
                winner = Some(ConflictWinner::Undecided);
            }
        }

        match winner {
            Some(winner) => {
                self.reply_tx.take().unwrap().send(winner).ok();

                ScreenCommand::CloseModal
            }
            None => ScreenCommand::Noop,
        }
    }
}
