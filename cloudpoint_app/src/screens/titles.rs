use cloudpoint_lib::ctr::SmdhLanguage;

use super::*;
use crate::{app::TaskMsg, db::TitleDb};
use std::{cmp, sync::mpsc::Sender};

pub struct TitlesScreen {
    task_tx: Sender<TaskMsg>,
    title_db: Option<TitleDb>,
    selected_idx: usize,
    show_from: usize,
}

impl TitlesScreen {
    pub fn new(task_tx: Sender<TaskMsg>) -> Self {
        task_tx.send(TaskMsg::BuildTitleDb).ok();

        Self {
            task_tx,
            title_db: None,
            selected_idx: 0,
            show_from: 0,
        }
    }
}

const PAGE_SIZE: usize = 12;

impl Screen for TitlesScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        ctx.rect(0.0, 0.0, TOP_W, TOP_H, WHITE);
        ctx.rect(0.0, 0.0, TOP_W, 32.0, ACCENT);
        ctx.text_centered(0.0, 6.0, TOP_W, 0.7, WHITE, "Title List");

        if let Some(title_db) = &self.title_db {
            for (view_idx, (item_idx, game_detail)) in title_db
                .titles()
                .enumerate()
                .skip(self.show_from)
                .take(PAGE_SIZE)
                .enumerate()
            {
                let mut colour = BLACK;
                if item_idx == self.selected_idx {
                    colour = WHITE;
                    ctx.rect(10.0, 40.0 + (view_idx * 16) as f32, 240.0, 16.0, ACCENT);
                }
                ctx.text(
                    12.0,
                    40.0 + (view_idx * 16) as f32,
                    0.5,
                    colour,
                    &game_detail.smdh.title_short(SmdhLanguage::English),
                );
            }
        }
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        ctx.rect(0.0, 0.0, BOT_W, BOT_H, ACCENT);
    }
}

impl BaseScreen for TitlesScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Titles
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
        if keys_down.intersects(KeyPad::DPAD_UP | KeyPad::CPAD_UP | KeyPad::CSTICK_UP) {
            self.selected_idx = self.selected_idx.saturating_sub(1);
        } else if keys_down.intersects(KeyPad::DPAD_LEFT | KeyPad::CPAD_LEFT | KeyPad::CSTICK_LEFT)
        {
            self.selected_idx = self.selected_idx.saturating_sub(PAGE_SIZE - 1);
        } else if keys_down.intersects(KeyPad::DPAD_DOWN | KeyPad::CPAD_DOWN | KeyPad::CSTICK_DOWN)
        {
            self.selected_idx = cmp::min(
                self.title_db.as_ref().unwrap().total_titles() - 1,
                self.selected_idx + 1,
            );
        } else if keys_down
            .intersects(KeyPad::DPAD_RIGHT | KeyPad::CPAD_RIGHT | KeyPad::CSTICK_RIGHT)
        {
            self.selected_idx = cmp::min(
                self.title_db.as_ref().unwrap().total_titles() - 1,
                self.selected_idx + PAGE_SIZE - 1,
            );
        } else if keys_down.intersects(KeyPad::L | KeyPad::R) {
            return ScreenCommand::SwitchTo(ScreenId::Sync);
        }

        self.clamp_viewport();

        ScreenCommand::Noop
    }
}

impl TitlesScreen {
    fn clamp_viewport(&mut self) {
        if self.selected_idx < self.show_from {
            self.show_from = self.selected_idx;
        } else if self.selected_idx >= self.show_from + PAGE_SIZE {
            self.show_from = self.selected_idx + 1 - PAGE_SIZE;
        }
    }
}
