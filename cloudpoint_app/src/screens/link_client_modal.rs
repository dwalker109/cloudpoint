use crate::ctr_cfgi::format_friend_code_seed;

use super::*;
use std::sync::mpsc::Sender;

pub struct LinkClientModalScreen {
    friend_code: String,
    cancel_tx: Sender<()>,
    success: Option<bool>,
}

impl LinkClientModalScreen {
    pub fn new(fc: u64, cancel_tx: Sender<()>) -> Self {
        Self {
            friend_code: format_friend_code_seed(fc),
            cancel_tx,
            success: None,
        }
    }
}

impl Screen for LinkClientModalScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        ctx.rect(20.0, 20.0, TOP_W - 40.0, TOP_H - 40.0, WHITE);

        let (l1, l2, l3) = match self.success {
            Some(true) => (
                "User key received successfully - Cloudpoint will exit.",
                "It will use the new key on your next sync.",
                "\u{E008}".into(),
            ),
            Some(false) => (
                "Something went wrong receiving your user key.",
                "Please try again.",
                "\u{E00A}".into(),
            ),
            None => (
                "Waiting for the other console.",
                "Confirm the codes match before continuing.",
                self.friend_code.clone(),
            ),
        };

        ctx.text_centered(0.0, 80.0, TOP_W, 0.5, BLACK, &l1);
        ctx.text_centered(0.0, 100.0, TOP_W, 0.5, BLACK, &l2);
        ctx.text_centered(0.0, 140.0, TOP_W, 1.2, ACCENT, &l3);
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        ctx.rect(20.0, 20.0, BOT_W - 40.0, BOT_H - 40.0, WHITE);

        match self.success {
            Some(_) => {
                ctx.text_centered(0.0, 110.0, BOT_W, 0.7, ACCENT, &"\u{E000} Continue");
            }
            None => {
                ctx.text_centered(0.0, 110.0, BOT_W, 0.7, ACCENT, &"\u{E001} Cancel");
            }
        }
    }
}

impl ModalScreen for LinkClientModalScreen {
    fn handle_msg(&mut self, msg: &UiMsg) -> ScreenCommand {
        match msg {
            UiMsg::LinkClientDone { success } => {
                self.success = Some(*success);
            }
            _ => {}
        }

        ScreenCommand::Noop
    }

    fn handle_input(&mut self, keys_down: &KeyPad, _keys_held: &KeyPad) -> ScreenCommand {
        match self.success {
            Some(true) if keys_down == &KeyPad::A => {
                self.cancel_tx.send(()).ok();
                ScreenCommand::RestartApp
            }
            Some(false) if keys_down == &KeyPad::A => {
                self.cancel_tx.send(()).ok();
                ScreenCommand::CloseModal
            }
            None => {
                if keys_down == &KeyPad::B {
                    self.cancel_tx.send(()).ok();
                    ScreenCommand::CloseModal
                } else {
                    ScreenCommand::Noop
                }
            }
            _ => ScreenCommand::Noop,
        }
    }
}
