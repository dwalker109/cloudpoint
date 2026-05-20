use crate::{app::UiMsg, ctr_gfx::*};
pub use conflict_modal::ConflictModalScreen;
use ctru::prelude::KeyPad;
pub use error_modal::ErrorModalScreen;
pub use help::HelpScreen;
pub use refresh_modal::RefreshModalScreen;
pub use sync::SyncScreen;
pub use sync_modal::SyncModalScreen;
pub use titles::TitlesScreen;

mod conflict_modal;
mod error_modal;
mod help;
mod refresh_modal;
mod shared;
mod sync;
mod sync_modal;
mod titles;

#[derive(Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScreenId {
    Sync,
    Titles,
    Help,
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
    fn handle_msg(&mut self, msg: &UiMsg) -> ScreenCommand;
    fn handle_input(&mut self, keys_down: &KeyPad, keys_held: &KeyPad) -> ScreenCommand;
}

pub trait ModalScreen: Screen {
    fn handle_msg(&mut self, msg: &UiMsg) -> ScreenCommand;
    fn handle_input(&mut self, keys_down: &KeyPad, keys_held: &KeyPad) -> ScreenCommand;
}
