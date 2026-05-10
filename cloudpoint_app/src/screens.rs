use crate::{app::UiMsg, ctr_gfx::*};
pub use conflict::ConflictModalScreen;
use ctru::prelude::KeyPad;
pub use games::GamesScreen;
pub use sync::SyncScreen;

mod games;
mod sync;
mod settings {}
mod conflict;

#[derive(Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScreenId {
    Sync,
    Games,
    Settings,
    ConflictModal,
}

pub enum ScreenCommand {
    SwitchTo(ScreenId),
    OpenModal(Box<dyn ModalScreen>),
    CloseModal,
    Noop,
}

pub trait Screen {
    fn draw_upper(&self, ctx: &DrawContext);
    fn draw_lower(&self, ctx: &DrawContext);
}

pub trait BaseScreen: Screen {
    fn id(&self) -> ScreenId;
    fn handle_msg(&mut self, msg: &UiMsg);
    fn handle_input(&mut self, input: &KeyPad) -> ScreenCommand;
}

pub trait ModalScreen: Screen {
    fn handle_input(&mut self, input: &KeyPad) -> ScreenCommand;
}
