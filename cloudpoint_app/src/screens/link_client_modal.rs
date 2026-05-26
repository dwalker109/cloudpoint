use crate::{
    ctr_cfgi::format_friend_code_seed,
    link::{LinkState, SharePermission},
};

use super::*;
use std::sync::mpsc::Sender;

pub struct LinkClientModalScreen {
    state: LinkState,
    friend_code: String,
    new_user_key: String,
    reply_tx: Option<Sender<SharePermission>>,
    cancel_tx: Sender<()>,
}

impl LinkClientModalScreen {
    pub fn new(fc: u64, cancel_tx: Sender<()>) -> Self {
        Self {
            state: LinkState::Init,
            friend_code: format_friend_code_seed(fc),
            new_user_key: String::new(),
            reply_tx: None,
            cancel_tx,
        }
    }
}

impl Screen for LinkClientModalScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        ctx.rect(20.0, 20.0, TOP_W - 40.0, TOP_H - 40.0, WHITE);

        ctx.text_centered(0.0, 40.0, TOP_W, 1.2, ACCENT, "\u{E075} \u{E01A}");

        let (l1, l2, l3) = match self.state {
            LinkState::Succeeded => (
                "User key received successfully - Cloudpoint will restart.",
                "It will use the new key on your next sync.",
                "\u{E008}",
            ),
            LinkState::Failed => (
                "Something went wrong receiving your user key.",
                "Please try again.",
                "\u{E00A}",
            ),
            LinkState::WaitingClient(..) => (
                "Are you sure you want to replace your user key?",
                "Make a choice with \u{E000} or \u{E001}",
                "\u{E011}",
            ),
            LinkState::Init => (
                "Waiting for the other console.",
                "Confirm this code is displayed on it before continuing.",
                self.friend_code.as_str(),
            ),
            LinkState::WaitingHost(..) => unreachable!(),
        };

        ctx.text_centered(0.0, 100.0, TOP_W, 0.5, BLACK, &l1);
        ctx.text_centered(0.0, 120.0, TOP_W, 0.5, BLACK, &l2);
        ctx.text_centered(0.0, 160.0, TOP_W, 1.2, ACCENT, &l3);
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        ctx.rect(20.0, 20.0, BOT_W - 40.0, BOT_H - 40.0, WHITE);

        match self.state {
            LinkState::Init => {
                ctx.text_centered(0.0, 110.0, BOT_W, 0.7, ACCENT, "\u{E001} Cancel");
            }
            LinkState::WaitingClient(..) => {
                ctx.text_centered(0.0, 90.0, BOT_W, 0.7, ACCENT, &"\u{E000} Allow");
                ctx.text_centered(0.0, 120.0, BOT_W, 0.7, ACCENT, &"\u{E001} Deny");
            }
            LinkState::Succeeded | LinkState::Failed => {
                ctx.text_centered(0.0, 110.0, BOT_W, 0.7, ACCENT, "\u{E000} Continue");
            }
            LinkState::WaitingHost(..) => unreachable!(),
        }
    }
}

impl ModalScreen for LinkClientModalScreen {
    fn handle_msg(&mut self, msg: &UiMsg) -> ScreenCommand {
        match msg {
            UiMsg::LinkClientConfirm {
                state,
                new_user_key,
                reply_tx,
            } => {
                self.state = *state;
                self.new_user_key = new_user_key.clone();
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
        match self.state {
            LinkState::WaitingClient(..) => {
                if keys_down == &KeyPad::A {
                    self.reply_tx
                        .take()
                        .and_then(|t| t.send(SharePermission::Allow).ok());
                } else if keys_down == &KeyPad::B {
                    self.reply_tx
                        .take()
                        .and_then(|t| t.send(SharePermission::Deny).ok());
                }
                ScreenCommand::Noop
            }
            LinkState::Succeeded if keys_down == &KeyPad::A => {
                self.cancel_tx.send(()).ok();
                ScreenCommand::RestartApp
            }
            LinkState::Failed if keys_down == &KeyPad::A => {
                self.cancel_tx.send(()).ok();
                ScreenCommand::CloseModal
            }
            LinkState::Init if keys_down == &KeyPad::B => {
                self.cancel_tx.send(()).ok();
                ScreenCommand::CloseModal
            }
            _ => ScreenCommand::Noop,
        }
    }
}
