/// These titles don't support sync to another system so are added but not enabled
/// during discovery. They *can* be synced if later enabled manually.
pub const UNSUPPORTED_TITLE_IDS: [u64; 1] = [
    // Super Mario Maker
    0x00040000001A0500,
];

pub use state::StateDb;
pub use title::TitleDb;

mod state;
mod title;
