use std::{
    sync::mpsc::{self, Sender},
    thread::sleep,
    time::{Duration, Instant},
};

use anyhow::{Result, bail};
use ctru::services::uds::{ConnectionType, NodeID, SendFlags, Uds};
use uuid::Uuid;

use crate::{
    app::{OpenModalMsg, UiMsg},
    config::{USER_KEY, backup_user_key, persist_user_key},
    ctr_cfgi::get_friend_code_seed,
};

pub enum SharePermission {
    Allow,
    Deny,
}

const COMM_ID: &'static [u8; 4] = b"CLPT";
const PASSPHRASE: &'static [u8; 10] = b"cloudpoint";
const CHANNEL: u8 = 1;
const CTRL_HELLO: u8 = 0x01;
const CTRL_ACK: u8 = 0x02;
const CTRL_HEREUGO: u8 = 0x03;
const CTRL_KTHXBYE: u8 = 0x04;

pub fn host(ui_tx: &Sender<UiMsg>, modal_tx: &Sender<OpenModalMsg>) -> Result<()> {
    let (quit_tx, quit_rx) = mpsc::channel::<()>();
    modal_tx.send(OpenModalMsg::LinkHost { quit_tx })?;

    let mut uds = Uds::new(None)?;
    uds.create_network(COMM_ID, None, Some(2), PASSPHRASE, CHANNEL)?;

    let mut peered = false;
    let timer = Instant::now();

    loop {
        if let Err(mpsc::TryRecvError::Disconnected) = quit_rx.try_recv() {
            return Ok(());
        }

        if timer.elapsed().as_secs() > 60 {
            bail!("timed out waiting for client");
        }

        if !peered {
            uds.send_packet(&[CTRL_HELLO], NodeID::Broadcast, 1, SendFlags::Default)?;
        }

        match uds.pull_packet() {
            Ok(Some((buf, reply_node))) if buf[0] == CTRL_ACK => {
                peered = true;
                uds.allow_new_clients(false)?;

                let fc = u64::from_le_bytes(buf[1..].try_into()?);
                let (reply_tx, reply_rx) = mpsc::channel::<SharePermission>();
                ui_tx.send(UiMsg::LinkHostConfirm { fc, reply_tx })?;

                match reply_rx.recv_timeout(Duration::from_secs(60)) {
                    Ok(SharePermission::Allow) => {
                        let mut pkt = [0x00; 17];
                        pkt[0] = CTRL_HEREUGO;
                        pkt[1..].copy_from_slice(USER_KEY.as_bytes());
                        uds.send_packet(&pkt, reply_node, 1, SendFlags::Default)?;
                        ui_tx.send(UiMsg::LinkHostDone { success: true })?;
                    }
                    Ok(SharePermission::Deny) => {
                        uds.eject_client(reply_node)?;
                        ui_tx.send(UiMsg::LinkHostDone { success: false })?;
                    }
                    Err(e) => {
                        bail!(e);
                    }
                }
            }
            Ok(Some((buf, _reply_node))) if buf[0] == CTRL_KTHXBYE => {
                break;
            }
            Ok(_) => {
                sleep(Duration::from_millis(500));
            }
            Err(e) => {
                bail!(e);
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

    let timer = Instant::now();
    let network = loop {
        if let Err(mpsc::TryRecvError::Disconnected) = quit_rx.try_recv() {
            return Ok(());
        }

        if timer.elapsed().as_secs() > 60 {
            bail!("could not find a find a network to join");
        }

        match uds.scan(COMM_ID, None, None)?.first() {
            Some(n) => break n.clone(),
            None => sleep(Duration::from_millis(500)),
        };
    };

    uds.connect_network(&network, PASSPHRASE, ConnectionType::Client, CHANNEL)?;
    let mut peered = false;

    let timer = Instant::now();
    loop {
        if let Err(mpsc::TryRecvError::Disconnected) = quit_rx.try_recv() {
            return Ok(());
        }

        if timer.elapsed().as_secs() > 60 {
            bail!("timed out waiting for host");
        }

        match uds.pull_packet() {
            Ok(Some((buf, reply_node))) if buf[0] == CTRL_HELLO && !peered => {
                peered = true;

                let mut pkt = [0x00; 9];
                pkt[0] = CTRL_ACK;
                pkt[1..].copy_from_slice(&fc.to_le_bytes());
                uds.send_packet(&pkt, reply_node, CHANNEL, SendFlags::Default)?;
            }
            Ok(Some((buf, reply_node))) if buf[0] == CTRL_HEREUGO => {
                backup_user_key()?;
                persist_user_key(Uuid::from_slice(&buf[1..17])?)?;
                ui_tx.send(UiMsg::LinkClientDone { success: true })?;
                uds.send_packet(&[CTRL_KTHXBYE], reply_node, CHANNEL, SendFlags::Default)?;
                break;
            }
            Ok(_) => {
                sleep(Duration::from_millis(500));
                continue;
            }
            Err(e) => {
                bail!(e);
            }
        }
    }

    Ok(())
}
