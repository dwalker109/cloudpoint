use crate::{app::UiMsg, ctr_gfx::*};
pub use conflict_modal::ConflictModalScreen;
use ctru::prelude::KeyPad;
pub use error_modal::ErrorModalScreen;
pub use link::LinkScreen;
pub use link_client_modal::LinkClientModalScreen;
pub use link_host_modal::LinkHostModalScreen;
pub use refresh_modal::RefreshModalScreen;
pub use shutdown_modal::ShutdownModalScreen;
pub use sync::SyncScreen;
pub use sync_modal::SyncModalScreen;
pub use titles::TitlesScreen;

mod conflict_modal;
mod error_modal;
mod link;
mod link_client_modal;
mod link_host_modal;
mod refresh_modal;
mod shared;
mod shutdown_modal;
mod sync;
mod sync_modal;
mod titles;

#[derive(Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScreenId {
    Sync,
    Titles,
    Link,
}

pub enum ScreenCommand {
    SwitchTo(ScreenId),
    OpenModal(Box<dyn ModalScreen>),
    CloseModal,
    RestartApp,
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
