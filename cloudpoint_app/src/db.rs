use crate::app::{OpenModalMsg, UiMsg};
use anyhow::Result;
pub use install_history::{InstallHistoryDb, InstallStatus};
use serde::Serialize;
pub use state::StateDb;
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
    sync::mpsc::Sender,
};
pub use title::{TitleDb, TitleDetails};

mod install_history;
mod state;
mod title;

pub static COMMIT_MSG: &'static str = "should commit db to sd card";

pub fn load(
    ui_tx: &Sender<UiMsg>,
    modal_tx: &Sender<OpenModalMsg>,
) -> Result<(TitleDb, StateDb, InstallHistoryDb)> {
    let (title_db, state_db) = match (TitleDb::open(), StateDb::open()) {
        (Ok(mut title_db), Ok(mut state_db)) => {
            if title_db.is_stale() {
                modal_tx.send(OpenModalMsg::Refresh).ok();
                title_db.refresh(&ui_tx);
                title_db.commit()?;

                modal_tx.send(OpenModalMsg::Refresh).ok();
                state_db.refresh(true, &title_db, &ui_tx);
                state_db.commit()?;
            }

            (title_db, state_db)
        }
        (Ok(mut title_db), Err(_)) => {
            if title_db.is_stale() {
                modal_tx.send(OpenModalMsg::Refresh).ok();
                title_db.refresh(&ui_tx);
                title_db.commit()?;
            }

            modal_tx.send(OpenModalMsg::Refresh).ok();
            let mut state_db = StateDb::new(&title_db, &ui_tx)?;
            state_db.commit()?;

            (title_db, state_db)
        }
        (Err(_), Ok(mut state_db)) => {
            modal_tx.send(OpenModalMsg::Refresh).ok();
            let mut title_db = TitleDb::new(&ui_tx)?;
            title_db.commit()?;

            modal_tx.send(OpenModalMsg::Refresh).ok();
            state_db.refresh(true, &title_db, &ui_tx);
            state_db.commit()?;

            (title_db, state_db)
        }
        (Err(_), Err(_)) => {
            modal_tx.send(OpenModalMsg::Refresh).ok();
            let mut title_db = TitleDb::new(&ui_tx)?;
            title_db.commit()?;

            modal_tx.send(OpenModalMsg::Refresh).ok();
            let mut state_db = StateDb::new(&title_db, &ui_tx)?;
            state_db.commit()?;

            (title_db, state_db)
        }
    };

    let install_history_db = match InstallHistoryDb::open() {
        Ok(install_history_db) => install_history_db,
        Err(_) => {
            let mut install_history_db = InstallHistoryDb::new()?;
            install_history_db.commit()?;

            install_history_db
        }
    };

    Ok((title_db, state_db, install_history_db))
}

const MAGIC: [u8; 4] = *b"CPDB";

fn decode_parts(buf: &[u8]) -> Result<(u16, &[u8])> {
    Ok(match buf.split_first_chunk::<6>() {
        Some((head, rest)) if head[..4] == MAGIC => {
            (u16::from_le_bytes(head[4..].try_into()?), rest)
        }
        _ => (0, buf),
    })
}

fn write_to_disk(path: impl AsRef<Path>, ver: u16, db: &impl Serialize) -> Result<()> {
    let mut w = BufWriter::new(File::create(&path)?);

    w.write_all(&MAGIC)?;
    w.write_all(&ver.to_le_bytes())?;
    postcard::to_io(db, &mut w)?;
    w.flush()?;

    Ok(())
}
