use cloudpoint_lib::{ctr::SmdhLanguage, title::TitleDetails};

use super::*;
use crate::{app::TaskMsg, db::TitleDb};
use std::sync::mpsc::Sender;

pub struct GamesScreen {
    task_tx: Sender<TaskMsg>,
    title_db: Option<TitleDb>,
    selected: usize,
    show_from: usize,
}

impl GamesScreen {
    pub fn new(task_tx: Sender<TaskMsg>) -> Self {
        task_tx.send(TaskMsg::BuildTitleDb).ok();

        Self {
            task_tx,
            title_db: None,
            selected: 0,
            show_from: 0,
        }
    }
}

impl Screen for GamesScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        ctx.rect(0.0, 0.0, TOP_W, TOP_H, WHITE);
        ctx.rect(0.0, 0.0, TOP_W, 32.0, ACCENT);
        ctx.text_centered(0.0, 6.0, TOP_W, 0.7, WHITE, "Games List");

        if let Some(title_db) = &self.title_db {
            for (view_idx, (item_idx, game_detail)) in title_db
                .titles()
                .enumerate()
                .skip(self.show_from)
                .take(20)
                .enumerate()
            {
                ctx.text(
                    10.0,
                    40.0 + (view_idx * 16) as f32,
                    0.5,
                    BLACK,
                    &game_detail.smdh.title_short(SmdhLanguage::English),
                );
            }
        }
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        ctx.rect(0.0, 0.0, BOT_W, BOT_H, ACCENT);
    }
}

impl BaseScreen for GamesScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Games
    }

    fn handle_msg(&mut self, msg: &UiMsg) {
        match msg {
            UiMsg::TitleDbReady { title_db } => {
                self.title_db = Some(title_db.clone());
            }
            _ => {}
        }
    }

    fn handle_input(&mut self, keys_down: &KeyPad, _keys_held: &KeyPad) -> ScreenCommand {
        if keys_down.intersects(KeyPad::L | KeyPad::R) {
            ScreenCommand::SwitchTo(ScreenId::Sync)
        } else {
            ScreenCommand::Noop
        }
    }
}
