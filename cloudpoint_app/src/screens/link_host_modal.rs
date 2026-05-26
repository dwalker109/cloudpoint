use super::*;
use crate::link::{LinkState, SharePermission};
use std::sync::mpsc::Sender;

pub struct LinkHostModalScreen {
    state: LinkState,
    friend_code: String,
    reply_tx: Option<Sender<SharePermission>>,
    cancel_tx: Sender<()>,
}

impl LinkHostModalScreen {
    pub fn new(cancel_tx: Sender<()>) -> Self {
        Self {
            state: LinkState::Init,
            friend_code: "...".into(),
            reply_tx: None,
            cancel_tx,
        }
    }
}

impl Screen for LinkHostModalScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        ctx.rect(20.0, 20.0, TOP_W - 40.0, TOP_H - 40.0, WHITE);

        let (l1, l2, l3) = match self.state {
            LinkState::Init => (
                "Waiting for the other console.",
                "Confirm this code is displayed on it before continuing.",
                self.friend_code.as_str(),
            ),
            LinkState::WaitingHost(..) => (
                "Verify that this code matches the console you want to",
                "share with and make a choice with \u{E000} or \u{E001}",
                self.friend_code.as_str(),
            ),
            LinkState::WaitingClient(..) => (
                "User key shared with your other console.",
                "Check it to complete the process.",
                "\u{E00B}",
            ),
            LinkState::Succeeded => (
                "User key sharing is complete. The other console will",
                "sync with the saves on this console from now on.",
                "\u{E008}",
            ),
            LinkState::Failed => (
                "Something went wrong sharing your user key.",
                "Please try again.",
                "\u{E00A}",
            ),
        };

        ctx.text_centered(0.0, 80.0, TOP_W, 0.5, BLACK, &l1);
        ctx.text_centered(0.0, 100.0, TOP_W, 0.5, BLACK, &l2);
        ctx.text_centered(0.0, 140.0, TOP_W, 1.2, ACCENT, &l3);
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        ctx.rect(20.0, 20.0, BOT_W - 40.0, BOT_H - 40.0, WHITE);

        match self.state {
            LinkState::Init | LinkState::WaitingClient(..) => {
                ctx.text_centered(0.0, 110.0, BOT_W, 0.7, ACCENT, "\u{E001} Cancel");
            }
            LinkState::WaitingHost(..) => {
                ctx.text_centered(0.0, 90.0, BOT_W, 0.7, ACCENT, &"\u{E000} Allow");
                ctx.text_centered(0.0, 120.0, BOT_W, 0.7, ACCENT, &"\u{E001} Deny");
            }
            LinkState::Succeeded | LinkState::Failed => {
                ctx.text_centered(0.0, 110.0, BOT_W, 0.7, ACCENT, "\u{E000} Continue");
            }
        }
    }
}

impl ModalScreen for LinkHostModalScreen {
    fn handle_msg(&mut self, msg: &UiMsg) -> ScreenCommand {
        match msg {
            UiMsg::LinkHostConfirm {
                state,
                friend_code,
                reply_tx,
            } => {
                self.state = *state;
                self.friend_code = friend_code.clone();
                self.reply_tx = Some(reply_tx.clone());
            }
            UiMsg::LinkUpdate { state } => {
                self.state = *state;
            }
            _ => {}
        }

        ScreenCommand::Noop
    }

    fn handle_input(&mut self, keys_down: &KeyPad, _keys_held: &KeyPad) -> ScreenCommand {
        if matches!(self.state, LinkState::WaitingHost(..)) {
            if keys_down == &KeyPad::A {
                self.reply_tx
                    .take()
                    .and_then(|t| t.send(SharePermission::Allow).ok());
            }
            if keys_down == &KeyPad::B {
                self.reply_tx
                    .take()
                    .and_then(|t| t.send(SharePermission::Deny).ok());
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
