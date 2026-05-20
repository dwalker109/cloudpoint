use super::*;
use crate::{app::TaskMsg, db::TitleDetails};
use std::sync::mpsc::Sender;

pub struct TitlesScreen {
    task_tx: Sender<TaskMsg>,
    titles: Vec<TitleDetails>,
    selected_idx: usize,
    show_from: usize,
}

impl TitlesScreen {
    pub fn new(task_tx: Sender<TaskMsg>) -> Self {
        Self {
            task_tx,
            titles: Vec::new(),
            selected_idx: 0,
            show_from: 0,
        }
    }

    fn selected_title(&self) -> Option<&TitleDetails> {
        self.titles
            .iter()
            .enumerate()
            .find(|(idx, _)| idx == &self.selected_idx)
            .map(|(_, title)| title)
    }

    fn max_idx(&self) -> usize {
        self.titles.len().saturating_sub(1)
    }
}

const PAGE_SIZE: usize = 12;

impl Screen for TitlesScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        ctx.rect(0.0, 0.0, TOP_W, TOP_H, WHITE);
        ctx.rect(0.0, 0.0, TOP_W, 32.0, ACCENT);
        ctx.icon(ICON_LIST, (TOP_W / 2.0) - 16.0, 0.0, 1.0);
        ctx.text(6.0, 0.0, 1.0, WHITE, "\u{E004}");
        ctx.text(TOP_W - 28.0, 0.0, 1.0, WHITE, "\u{E005}");

        if !self.titles.is_empty() {
            for (view_idx, (item_idx, game_detail)) in self
                .titles
                .iter()
                .enumerate()
                .skip(self.show_from)
                .take(PAGE_SIZE)
                .enumerate()
            {
                let mut colour = BLACK;
                if item_idx == self.selected_idx {
                    colour = WHITE;
                    ctx.rect(
                        10.0,
                        40.0 + (view_idx * 16) as f32,
                        TOP_W - 20.0,
                        16.0,
                        ACCENT,
                    );
                }
                ctx.text(
                    12.0,
                    40.0 + (view_idx * 16) as f32,
                    0.5,
                    colour,
                    &game_detail.title_short,
                );
            }
        } else {
            ctx.text_centered(0.0, 116.0, TOP_W, 0.6, BLACK, &"No titles found.");
            ctx.text_centered(0.0, 136.0, TOP_W, 0.6, BLACK, &"Add some and come back!");
        }
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        ctx.rect(0.0, 0.0, BOT_W, BOT_H / 2.0, ACCENT);
        ctx.rect(0.0, BOT_H / 2.0, BOT_W, BOT_H / 2.0, WHITE);

        let Some(title) = self.selected_title() else {
            return;
        };

        ctx.text(12.0, 12.0, 0.7, WHITE, &title.title_short);
        ctx.text(12.0, 36.0, 0.7, WHITE, &title.title_publisher);
        ctx.text(
            12.0,
            60.0,
            0.7,
            WHITE,
            &format!("{:05X}", (title.title_id >> 8) as u32),
        );
        ctx.text(12.0, 84.0, 0.7, WHITE, &title.product_code);

        ctx.text_centered(
            0.0,
            132.0,
            BOT_W,
            0.55,
            BLACK,
            &format!("Save auto sync: {}", title.savedata_sync_status),
        );
        ctx.text_centered(
            0.0,
            150.0,
            BOT_W,
            0.55,
            BLACK,
            &format!("Extdata auto sync: {}", title.extdata_sync_status),
        );

        ctx.text(
            20.0,
            178.0,
            0.7,
            ACCENT,
            "\u{E000} Run sync now for this title".into(),
        );
        ctx.text(
            20.0,
            200.0,
            0.7,
            ACCENT,
            "\u{E003} Toggle auto sync for this title".into(),
        );
    }
}

impl BaseScreen for TitlesScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Titles
    }

    fn handle_msg(&mut self, msg: &UiMsg) -> ScreenCommand {
        match msg {
            UiMsg::RefreshDone { titles, .. } => {
                self.titles = titles.clone();
            }
            _ => {}
        }

        ScreenCommand::Noop
    }

    fn handle_input(&mut self, keys_down: &KeyPad, _keys_held: &KeyPad) -> ScreenCommand {
        if keys_down.intersects(KeyPad::DPAD_UP | KeyPad::CPAD_UP | KeyPad::CSTICK_UP) {
            self.selected_idx = self
                .selected_idx
                .checked_sub(1)
                .or_else(|| Some(self.max_idx()))
                .unwrap_or_default();
        } else if keys_down.intersects(KeyPad::DPAD_DOWN | KeyPad::CPAD_DOWN | KeyPad::CSTICK_DOWN)
        {
            self.selected_idx = (self.selected_idx + 1) % (self.max_idx() + 1);
        } else if keys_down.intersects(KeyPad::L | KeyPad::R) {
            return ScreenCommand::SwitchTo(ScreenId::Sync);
        } else if keys_down.contains(KeyPad::A) {
            if let Some(title) = self.selected_title() {
                self.task_tx
                    .send(TaskMsg::SyncTargeted(title.title_id))
                    .ok();

                return ScreenCommand::OpenModal(Box::new(SyncModalScreen::new()));
            }
        } else if keys_down.contains(KeyPad::Y) {
            if let Some(title) = self.selected_title() {
                self.task_tx.send(TaskMsg::Toggle(title.title_id)).ok();
            }
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
