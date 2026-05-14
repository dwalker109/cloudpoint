use super::*;
use crate::{app::TaskMsg, db::TitleDetails};
use cloudpoint_lib::ctr::SmdhLanguage;
use std::{cmp, sync::mpsc::Sender};

pub struct TitlesScreen {
    task_tx: Sender<TaskMsg>,
    task_running: bool,
    titles: Option<Vec<TitleDetails>>,
    selected_idx: usize,
    show_from: usize,
}

impl TitlesScreen {
    pub fn new(task_tx: Sender<TaskMsg>) -> Self {
        task_tx.send(TaskMsg::TitleDbReady).ok();

        Self {
            task_tx,
            task_running: false,
            titles: None,
            selected_idx: 0,
            show_from: 0,
        }
    }

    fn selected_title(&self) -> Option<&TitleDetails> {
        self.titles.as_ref().and_then(|titles| {
            titles
                .iter()
                .enumerate()
                .find(|(idx, _)| idx == &self.selected_idx)
                .map(|(_, title)| title)
        })
    }
}

const PAGE_SIZE: usize = 12;

impl Screen for TitlesScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        ctx.rect(0.0, 0.0, TOP_W, TOP_H, WHITE);
        ctx.rect(0.0, 0.0, TOP_W, 32.0, ACCENT);
        ctx.text_centered(0.0, 6.0, TOP_W, 0.7, WHITE, "Titles");

        if let Some(titles) = &self.titles {
            for (view_idx, (item_idx, game_detail)) in titles
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
                    &game_detail.smdh.title_short(SmdhLanguage::English),
                );
            }
        }
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        ctx.rect(0.0, 0.0, BOT_W, BOT_H, ACCENT);
        let Some(title) = self.selected_title() else {
            ctx.text_centered(0.0, 110.0, BOT_W, 0.6, BLACK, &"Loading titles...");

            return;
        };

        let title_short = title.smdh.title_short(SmdhLanguage::English);
        let title_publisher = title.smdh.title_publisher(SmdhLanguage::English);

        ctx.text(
            12.0,
            12.0,
            0.5,
            BLACK,
            &format!("{} | {:016X}", title.product_code, title.title_id),
        );
        ctx.text(12.0, 28.0, 0.5, BLACK, &title_short);
        ctx.text(12.0, 44.0, 0.5, BLACK, &title_publisher);

        ctx.text(
            12.0,
            80.0,
            0.5,
            BLACK,
            &format!("Include save in auto sync: {}", title.savedata_sync_status),
        );
        ctx.text(
            12.0,
            96.0,
            0.5,
            BLACK,
            &format!(
                "Include extdata in auto sync: {}",
                title.extdata_sync_status
            ),
        );
    }
}

impl BaseScreen for TitlesScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Titles
    }

    fn handle_msg(&mut self, msg: &UiMsg) {
        match msg {
            UiMsg::TitleDbReady { titles } => {
                self.task_running = false;
                self.titles = Some(titles.clone());
            }
            UiMsg::TitleDbInvalidated => {
                if !self.task_running {
                    self.task_running = true;
                    self.titles = None;
                    self.task_tx.send(TaskMsg::TitleDbReady).ok();
                }
            }
            _ => {}
        }
    }

    fn handle_input(&mut self, keys_down: &KeyPad, _keys_held: &KeyPad) -> ScreenCommand {
        if keys_down.intersects(KeyPad::DPAD_UP | KeyPad::CPAD_UP | KeyPad::CSTICK_UP) {
            self.selected_idx = self.selected_idx.saturating_sub(1);
        } else if keys_down.intersects(KeyPad::DPAD_DOWN | KeyPad::CPAD_DOWN | KeyPad::CSTICK_DOWN)
        {
            self.selected_idx = cmp::min(
                self.titles
                    .as_ref()
                    .and_then(|t| Some(t.len()))
                    .unwrap_or_default()
                    .saturating_sub(1),
                self.selected_idx + 1,
            );
        } else if keys_down.intersects(KeyPad::L | KeyPad::R) {
            return ScreenCommand::SwitchTo(ScreenId::Sync);
        } else if keys_down.contains(KeyPad::A) {
            if let Some(title) = self.selected_title() {
                self.task_tx
                    .send(TaskMsg::DiscoverTargeted(title.title_id))
                    .ok();

                self.task_tx
                    .send(TaskMsg::SyncTargeted(
                        [title.savedata_sync_item, title.extdata_sync_item]
                            .iter()
                            .flatten()
                            .copied()
                            .collect(),
                    ))
                    .ok();
            }

            return ScreenCommand::OpenModal(Box::new(SyncModalScreen::new()));
        } else if keys_down.contains(KeyPad::Y) {
            if let Some(title) = self.selected_title() {
                self.task_tx
                    .send(TaskMsg::DiscoverTargeted(title.title_id))
                    .ok();
                self.task_tx
                    .send(TaskMsg::ToggleTargeted(title.title_id))
                    .ok();
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
