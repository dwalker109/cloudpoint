use std::{
    sync::mpsc::{self, Sender},
    thread::sleep,
    time::Duration,
};

use anyhow::{Result, bail};
use ctru::services::uds::{ConnectionType, NodeID, SendFlags, Uds};
use uuid::Uuid;

use crate::{
    app::{OpenModalMsg, UiMsg},
    config::{USER_KEY, backup_user_key, persist_user_key},
    ctr_cfgi::{format_friend_code_seed, get_friend_code_seed},
};

#[derive(Eq, PartialEq, Copy, Clone)]
pub enum LinkState {
    Init,
    WaitingHost((NodeID, u64)),
    WaitingClient((NodeID, Uuid)),
    Failed,
    Succeeded,
}

pub enum SharePermission {
    Allow,
    Deny,
}

const COMM_ID: &'static [u8; 4] = b"CLPT";
const PASSPHRASE: &'static [u8; 10] = b"cloudpoint";
const CHANNEL: u8 = 1;
const CTRL_HELLO: u8 = 0x01;
const CTRL_ACK: u8 = 0x02;
const CTRL_PAYLOAD: u8 = 0x03;
const CTRL_FIN_OK: u8 = 0x04;
const CTRL_FIN_ERR: u8 = 0x05;

pub fn host(ui_tx: &Sender<UiMsg>, modal_tx: &Sender<OpenModalMsg>) -> Result<()> {
    let (quit_tx, quit_rx) = mpsc::channel::<()>();
    modal_tx.send(OpenModalMsg::LinkHost { quit_tx })?;

    let mut uds = Uds::new(None)?;
    uds.create_network(COMM_ID, None, Some(2), PASSPHRASE, CHANNEL)?;

    let (reply_tx, reply_rx) = mpsc::channel::<SharePermission>();
    let mut state = LinkState::Init;

    loop {
        if let Err(mpsc::TryRecvError::Disconnected) = quit_rx.try_recv() {
            if let LinkState::WaitingHost((client_node, ..)) = state {
                uds.send_packet(&[CTRL_FIN_ERR], client_node, CHANNEL, SendFlags::Default)?;
            }

            return Ok(());
        }

        if state == LinkState::Init {
            uds.send_packet(&[CTRL_HELLO], NodeID::Broadcast, 1, SendFlags::Default)?;
        }

        match uds.pull_packet() {
            Ok(Some((buf, client_node))) if buf[0] == CTRL_ACK => {
                uds.allow_new_clients(false)?;
                let fc = u64::from_le_bytes(buf[1..9].try_into()?);
                state = LinkState::WaitingHost((client_node, fc));
                ui_tx.send(UiMsg::LinkHostConfirm {
                    state,
                    friend_code: format_friend_code_seed(fc),
                    reply_tx: reply_tx.clone(),
                })?;
            }
            Ok(Some((buf, ..))) if buf[0] == CTRL_FIN_OK => {
                state = LinkState::Succeeded;
                ui_tx.send(UiMsg::LinkUpdate { state })?;
                break;
            }
            Ok(Some((buf, ..))) if buf[0] == CTRL_FIN_ERR => {
                bail!("other side hung up");
            }
            Err(e) => {
                bail!(e);
            }
            _ => {
                sleep(Duration::from_millis(100));
            }
        }

        if let Ok(response) = reply_rx.try_recv()
            && let LinkState::WaitingHost((client_node, fc)) = state
        {
            match response {
                SharePermission::Allow => {
                    let mut pkt = [0x00u8; 25];
                    pkt[0] = CTRL_PAYLOAD;
                    pkt[1..9].copy_from_slice(&fc.to_le_bytes());
                    pkt[9..25].copy_from_slice(USER_KEY.as_bytes());
                    uds.send_packet(&pkt, client_node, 1, SendFlags::Default)?;
                    state = LinkState::WaitingClient((NodeID::None, *USER_KEY));
                    ui_tx.send(UiMsg::LinkUpdate { state })?;
                }
                SharePermission::Deny => {
                    uds.eject_client(client_node)?;
                    bail!("share permission denied");
                }
            }
        }
    }

    Ok(())
}

pub fn client(ui_tx: &Sender<UiMsg>, modal_tx: &Sender<OpenModalMsg>) -> Result<()> {
    let (quit_tx, quit_rx) = mpsc::channel::<()>();
    let fc = get_friend_code_seed()?;
    modal_tx.send(OpenModalMsg::LinkClient { fc, quit_tx })?;

    let mut uds = Uds::new(None)?;

    let network = loop {
        if let Err(mpsc::TryRecvError::Disconnected) = quit_rx.try_recv() {
            return Ok(());
        }

        match uds.scan(COMM_ID, None, None)?.first() {
            Some(n) => break n.clone(),
            None => sleep(Duration::from_millis(100)),
        };
    };

    uds.connect_network(&network, PASSPHRASE, ConnectionType::Client, CHANNEL)?;

    let (reply_tx, reply_rx) = mpsc::channel::<SharePermission>();
    let mut state = LinkState::Init;

    loop {
        if let Err(mpsc::TryRecvError::Disconnected) = quit_rx.try_recv() {
            if let LinkState::WaitingClient((client_node, ..))
            | LinkState::WaitingHost((client_node, ..)) = state
            {
                uds.send_packet(&[CTRL_FIN_ERR], client_node, CHANNEL, SendFlags::Default)?;
            }

            return Ok(());
        }

        match uds.pull_packet() {
            Ok(Some((buf, host_node))) if buf[0] == CTRL_HELLO => {
                state = LinkState::WaitingHost((host_node, fc));
                let mut pkt = [0x00; 9];
                pkt[0] = CTRL_ACK;
                pkt[1..9].copy_from_slice(&fc.to_le_bytes());
                uds.send_packet(&pkt, host_node, CHANNEL, SendFlags::Default)?;
            }
            Ok(Some((buf, host_node))) if buf[0] == CTRL_PAYLOAD => {
                let session_fc = u64::from_le_bytes(buf[1..9].try_into()?);

                if session_fc != fc {
                    bail!("mismatched friend code and session friend code");
                }

                let new_user_key = Uuid::from_slice(&buf[9..25])?;
                state = LinkState::WaitingClient((host_node, new_user_key));
                ui_tx.send(UiMsg::LinkClientConfirm {
                    state,
                    new_user_key: new_user_key.as_simple().to_string(),
                    reply_tx: reply_tx.clone(),
                })?;
            }
            Ok(Some((buf, ..))) if buf[0] == CTRL_FIN_ERR => {
                bail!("other side hung up");
            }
            Err(e) => {
                bail!(e);
            }
            _ => {
                sleep(Duration::from_millis(100));
            }
        }

        if let Ok(response) = reply_rx.try_recv()
            && let LinkState::WaitingClient((client_node, new_user_key)) = state
        {
            match response {
                SharePermission::Allow => {
                    backup_user_key()?;
                    persist_user_key(new_user_key)?;
                    state = LinkState::Succeeded;
                    ui_tx.send(UiMsg::LinkUpdate { state })?;
                    uds.send_packet(&[CTRL_FIN_OK], client_node, CHANNEL, SendFlags::Default)?;
                    break;
                }
                SharePermission::Deny => {
                    state = LinkState::Failed;
                    ui_tx.send(UiMsg::LinkUpdate { state })?;
                    uds.send_packet(&[CTRL_FIN_ERR], client_node, CHANNEL, SendFlags::Default)?;
                    break;
                }
            }
        }
    }

    Ok(())
}
