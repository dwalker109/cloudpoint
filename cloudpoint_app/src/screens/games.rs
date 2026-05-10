use super::*;
use crate::app::TaskMsg;
use std::sync::mpsc::Sender;

pub struct GamesScreen {
    task_tx: Sender<TaskMsg>,
    discover_available: bool,
    upper_1: String,
    upper_2: String,
    lower_1: String,
}

impl GamesScreen {
    pub fn new(task_tx: Sender<TaskMsg>) -> Self {
        task_tx.send(TaskMsg::ReadyDiscover).ok();

        Self {
            task_tx,
            discover_available: true,
            upper_1: String::with_capacity(256),
            upper_2: String::with_capacity(256),
            lower_1: String::with_capacity(256),
        }
    }
}

impl Screen for GamesScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        ctx.rect(0.0, 0.0, TOP_W, TOP_H, WHITE);
        ctx.rect(0.0, 0.0, TOP_W, 32.0, ACCENT);
        ctx.text_centered(0.0, 6.0, TOP_W, 0.7, WHITE, "Games List");
        ctx.text_centered(0.0, 100.0, TOP_W, 0.6, BLACK, &self.upper_1);
        ctx.text_centered(0.0, 120.0, TOP_W, 0.6, BLACK, &self.upper_2);
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        ctx.rect(0.0, 0.0, BOT_W, BOT_H, ACCENT);
        ctx.text_centered(0.0, 110.0, BOT_W, 0.6, BLACK, &self.lower_1);
    }
}

impl BaseScreen for GamesScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Games
    }

    fn handle_msg(&mut self, msg: &UiMsg) {
        match msg {
            UiMsg::DiscoverReady { total_states } => {
                self.upper_1 = "Ready to discover".into();
                self.upper_2 = format!("{total_states} saves available");
                self.lower_1 = "Press (A) to discover new saves".into()
            }
            UiMsg::DiscoverDone { total_states } => {
                self.upper_1 = "Discover completed".into();
                self.upper_2 = format!("{total_states} saves available");
                self.lower_1 = "Discovered saves are up to date".into()
            }
            _ => {}
        }
    }

    fn handle_input(&mut self, keys: &KeyPad) -> ScreenCommand {
        if self.discover_available && keys.contains(KeyPad::A) {
            self.discover_available = false;
            self.lower_1 = "...".into();
            self.task_tx.send(TaskMsg::StartDiscover).ok();
            ScreenCommand::Noop
        } else if keys.intersects(KeyPad::L | KeyPad::R) {
            ScreenCommand::SwitchTo(ScreenId::Sync)
        } else {
            ScreenCommand::Noop
        }
    }
}
