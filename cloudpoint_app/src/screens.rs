use crate::{app::UiMsg, ctr_gfx::*};
pub use conflict::ConflictModalScreen;
use ctru::prelude::KeyPad;
pub use sync::SyncScreen;
pub use titles::TitlesScreen;

mod sync;
mod titles;
mod settings {}
mod conflict;

#[derive(Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScreenId {
    Sync,
    Titles,
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
    fn handle_input(&mut self, keys_down: &KeyPad, keys_held: &KeyPad) -> ScreenCommand;
}

pub trait ModalScreen: Screen {
    fn handle_input(&mut self, keys_down: &KeyPad, keys_held: &KeyPad) -> ScreenCommand;
}
