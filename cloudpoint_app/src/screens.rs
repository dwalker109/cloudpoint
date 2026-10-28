use crate::{app::UiMsg, gfx::*};
use ctru::prelude::KeyPad;

pub use base::connect::ConnectScreen;
pub use base::link::LinkScreen;
pub use base::sync::SyncScreen;
pub use base::titles::TitlesScreen;

pub use modal::conflict::ConflictModalScreen;
pub use modal::error::ErrorModalScreen;
pub use modal::link_client::LinkClientModalScreen;
pub use modal::link_host::LinkHostModalScreen;
pub use modal::refresh::RefreshModalScreen;
pub use modal::shutdown::ShutdownModalScreen;
pub use modal::sync::SyncModalScreen;
pub use modal::update::UpdateModalScreen;

mod base {
    use super::*;

    pub(super) mod connect;
    pub(super) mod link;
    pub(super) mod sync;
    pub(super) mod titles;
}

mod modal {
    use super::*;

    pub(super) mod conflict;
    pub(super) mod error;
    pub(super) mod link_client;
    pub(super) mod link_host;
    pub(super) mod refresh;
    pub(super) mod shutdown;
    pub(super) mod sync;
    pub(super) mod update;
}

mod shared;

#[derive(Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScreenId {
    Connect,
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

    fn handle_msg(&mut self, _msg: &UiMsg) -> ScreenCommand {
        ScreenCommand::Noop
    }

    fn handle_input(&mut self, _keys_down: &KeyPad, _keys_held: &KeyPad) -> ScreenCommand {
        ScreenCommand::Noop
    }
}

pub trait BaseScreen: Screen {
    fn id(&self) -> ScreenId;
}

pub trait ModalScreen: Screen {}
