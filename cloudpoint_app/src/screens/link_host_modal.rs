use std::sync::mpsc::Sender;

use crate::{ctr_cfgi::format_friend_code_seed, link::SharePermission};

use super::*;

pub struct LinkHostModalScreen {
    friend_code: Option<String>,
    reply_tx: Option<Sender<SharePermission>>,
    cancel_tx: Sender<()>,
    success: Option<bool>,
}

impl LinkHostModalScreen {
    pub fn new(cancel_tx: Sender<()>) -> Self {
        Self {
            friend_code: None,
            reply_tx: None,
            cancel_tx,
            success: None,
        }
    }
}

impl Screen for LinkHostModalScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        ctx.rect(20.0, 20.0, TOP_W - 40.0, TOP_H - 40.0, WHITE);

        let (l1, l2, l3) = match self.success {
            Some(true) => (
                "User key shared with your other console,",
                "Check it to complete the process.",
                "\u{E008}".into(),
            ),
            Some(false) => (
                "Something went wrong sharing your user key.",
                "Please try again.",
                "\u{E00A}".into(),
            ),
            None if self.reply_tx.is_some() => (
                "Verify the code matches the other",
                "console and make a choice with \u{E000} or \u{E001}",
                self.friend_code.clone().unwrap_or("...".into()),
            ),
            None => (
                "Waiting for the other console.",
                "Confirm the codes match before continuing.",
                self.friend_code.clone().unwrap_or("...".into()),
            ),
        };

        ctx.text_centered(0.0, 80.0, TOP_W, 0.5, BLACK, &l1);
        ctx.text_centered(0.0, 100.0, TOP_W, 0.5, BLACK, &l2);
        ctx.text_centered(0.0, 140.0, TOP_W, 1.2, ACCENT, &l3);
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        ctx.rect(20.0, 20.0, BOT_W - 40.0, BOT_H - 40.0, WHITE);

        if self.reply_tx.is_some() {
            ctx.text_centered(0.0, 90.0, BOT_W, 0.7, ACCENT, &"\u{E000} Allow");
            ctx.text_centered(0.0, 120.0, BOT_W, 0.7, ACCENT, &"\u{E001} Deny");
        } else {
            let msg = match self.success {
                Some(_) => "\u{E000} Continue",
                None => "\u{E001} Cancel",
            };
            ctx.text_centered(0.0, 110.0, BOT_W, 0.7, ACCENT, msg);
        }
    }
}

impl ModalScreen for LinkHostModalScreen {
    fn handle_msg(&mut self, msg: &UiMsg) -> ScreenCommand {
        match msg {
            UiMsg::LinkHostConfirm { fc, reply_tx } => {
                self.friend_code = Some(format_friend_code_seed(*fc));
                self.reply_tx = Some(reply_tx.clone());
            }
            UiMsg::LinkHostDone { success } => {
                self.success = Some(*success);
                self.reply_tx = None;
            }
            _ => {}
        }

        ScreenCommand::Noop
    }

    fn handle_input(&mut self, keys_down: &KeyPad, _keys_held: &KeyPad) -> ScreenCommand {
        if let Some(reply_tx) = self.reply_tx.as_ref() {
            if keys_down == &KeyPad::A {
                reply_tx.send(SharePermission::Allow).ok();
            }
            if keys_down == &KeyPad::B {
                reply_tx.send(SharePermission::Deny).ok();
            }
        } else {
            if keys_down.intersects(KeyPad::A | KeyPad::B) {
                self.cancel_tx.send(()).ok();
                return ScreenCommand::CloseModal;
            }
        };

        ScreenCommand::Noop
    }
}
