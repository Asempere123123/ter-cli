use std::{path::Path, time::Duration, u8};

use nix::net::if_::if_nameindex;
use probe_rs::{
    Error, Permissions, Session,
    probe::{DebugProbeError, DebugProbeInfo, ProbeCreationError, list::Lister},
};
use smol::{future::FutureExt, process::Command};
use socketcan::{
    CanDataFrame, CanFrame, CanInterface, EmbeddedFrame, Id, StandardId, smol::CanSocket,
};

use crate::{descriptor::Descriptor, flash_size};

pub const FLASH_BASE_ADDR: u64 = 0x08000000;

const FLASH_WRITE_SIZE: usize = 128;

const BEGIN_FLASH_MSG_ID: Id = Id::Standard(unsafe { StandardId::new_unchecked(22) });
const ACK_MSG_ID: Id = Id::Standard(unsafe { StandardId::new_unchecked(23) });
const RECEIVE_WORD_MSG_ID: Id = Id::Standard(unsafe { StandardId::new_unchecked(24) });
const REVERT_WORD_MSG_ID: Id = Id::Standard(unsafe { StandardId::new_unchecked(25) });
const FINISH_FLASH_MSG_ID: Id = Id::Standard(unsafe { StandardId::new_unchecked(26) });

pub fn flash(
    bootloader_path: impl AsRef<Path>,
    app_path: impl AsRef<Path>,
    chip_name: &str,
    bootloader_defmt: bool,
    can: bool,
    descriptor: &Descriptor,
) -> anyhow::Result<Session> {
    log::info!(
        "FLASH_ERASE_SIZE = {}",
        flash_size::get_first_sector_size(&descriptor)?
    );
    log::info!("Flashing App");
    let bootloader = std::fs::read(bootloader_path)?;
    let app = std::fs::read(app_path)?;

    if can {
        smol::block_on(flash_can(descriptor, app))?;
        // No puede retornar la sesion, no hay nada mas que hacer (pq no esta enchufado)
        std::process::exit(0);
    }

    let mut session = get_session(chip_name)?;
    let mut loader = session.target().flash_loader();

    loader.add_data(FLASH_BASE_ADDR, &bootloader)?;
    if !bootloader_defmt {
        loader.add_data(
            FLASH_BASE_ADDR + flash_size::get_first_sector_size(&descriptor)?,
            &app,
        )?;
    }
    loader.commit(&mut session, Default::default())?;

    log::info!("Flashing Complete. Reseting app");
    let mut core = session.core(0)?;
    core.reset()?;
    drop(core);

    Ok(session)
}

fn attach_to_core(probe: &DebugProbeInfo, chip_name: &str) -> anyhow::Result<Session> {
    for _i in 0..2 {
        if let Ok(session) = probe.open()?.attach(chip_name, Permissions::default()) {
            return Ok(session);
        }

        if let Ok(session) = probe
            .open()?
            .attach_under_reset(chip_name, Permissions::default())
        {
            return Ok(session);
        }
    }

    Ok(probe.open()?.attach(chip_name, Permissions::default())?)
}

pub fn get_session(chip_name: &str) -> anyhow::Result<Session> {
    let lister = Lister::new();
    let probes = lister.list_all();
    let probe = probes
        .first()
        .ok_or(Error::Probe(DebugProbeError::ProbeCouldNotBeCreated(
            ProbeCreationError::NotFound,
        )))?;
    attach_to_core(probe, chip_name)
}

fn get_can_interface() -> anyhow::Result<String> {
    if_nameindex()
        .ok()
        .iter()
        .flatten()
        .filter_map(|interface| {
            interface
                .name()
                .to_str()
                .ok()
                .filter(|name| name.starts_with("can"))
        })
        .map(|name| name.to_owned())
        .next()
        .ok_or(anyhow::anyhow!("No can interface found")) // TODO: For now just select the first available one
}

async fn configure_can(baudrate: u32) -> anyhow::Result<String> {
    let can_interface = get_can_interface()?;

    let iface = CanInterface::open(&can_interface)?;

    let bittiming = iface.bit_timing()?;
    let current_state = iface.details()?;

    let is_correct_rate = bittiming.map(|b| b.bitrate == baudrate).unwrap_or(false);

    if current_state.is_up && is_correct_rate {
        return Ok(can_interface);
    }

    // It can also be done from within the socketcan lib but it requires root
    // TODO: This is probably an attack vector and a bad idea not to sanity check
    let cmd_script = format!(
        "ip link set {iface} down && \
         ip link set {iface} type can bitrate {rate} && \
         ip link set {iface} up",
        iface = can_interface,
        rate = baudrate
    );

    // 3. Execute via sudo
    let _status = Command::new("sudo")
        .arg("sh")
        .arg("-c")
        .arg(&cmd_script)
        .status()
        .await?
        .success();

    Ok(can_interface)
}

async fn flash_can(descriptor: &Descriptor, mut app: Vec<u8>) -> anyhow::Result<()> {
    app.resize(app.len().div_ceil(1024) * 1024, u8::MAX);
    let can_iface = configure_can(descriptor.can_baudrate().ok_or(anyhow::anyhow!(
        "No can baudrate specified. Have you set up the can stack in the ter.toml file?"
    ))?)
    .await?;

    let can_socket = CanSocket::open(&can_iface)?;

    // Empieza zona desastre
    let warn_task_handle = smol::spawn(async {
        loop {
            smol::Timer::after(Duration::from_secs(1)).await;
            log::warn!("Waiting for chip to enter bootloader mode");
        }
    });

    loop {
        let data_frame =
            CanDataFrame::new(BEGIN_FLASH_MSG_ID, &descriptor.name_hash().to_le_bytes())
                .ok_or(anyhow::anyhow!("Invalid dataframe produced"))?;
        can_socket.write_frame(&CanFrame::Data(data_frame)).await?;

        let enter_flashing_res = async {
            loop {
                let Ok(frame) = can_socket.read_frame().await else {
                    continue;
                };
                if frame.id() == ACK_MSG_ID {
                    break EnterFlashingResult::AckReceived;
                }
            }
        }
        .or(async {
            smol::Timer::after(Duration::from_millis(100));
            EnterFlashingResult::TimedOut
        })
        .await;

        if enter_flashing_res == EnterFlashingResult::AckReceived {
            break;
        }
    }

    log::info!("ACK Received. Chip now in bootloader mode");
    warn_task_handle.cancel().await;
    // Termina zona desastre

    CanFlasher::new(app, can_socket).flash_to_end().await?;

    Ok(())
}

#[derive(PartialEq)]
enum EnterFlashingResult {
    AckReceived,
    TimedOut,
}

struct CanFlasher {
    app: Vec<u8>,
    can: CanSocket,
    current_offset: usize,
}

impl CanFlasher {
    pub fn new(app: Vec<u8>, can: CanSocket) -> Self {
        Self {
            app,
            can,
            current_offset: 0,
        }
    }

    pub async fn flash_to_end(mut self) -> anyhow::Result<()> {
        while self.current_offset < self.app.len() {
            self.flash().await?;
        }

        let frame = CanFrame::Data(
            CanDataFrame::new(FINISH_FLASH_MSG_ID, &[])
                .ok_or(anyhow::anyhow!("Could not create can frame"))?,
        );
        self.can.write_frame(&frame).await?;

        Ok(())
    }

    async fn flash(&mut self) -> anyhow::Result<()> {
        for curr_offset in
            (self.current_offset..(self.current_offset + FLASH_WRITE_SIZE)).step_by(8)
        {
            let frame = CanFrame::Data(
                CanDataFrame::new(
                    RECEIVE_WORD_MSG_ID,
                    &self.app[curr_offset..(curr_offset + 8)],
                )
                .ok_or(anyhow::anyhow!("Could not create can frame"))?,
            );
            self.can.write_frame(&frame).await?;
        }

        let ack_received = async {
            loop {
                let Ok(frame) = self.can.read_frame().await else {
                    continue;
                };

                if frame.id() == ACK_MSG_ID {
                    break EnterFlashingResult::AckReceived;
                }
            }
        }
        .or(async {
            smol::Timer::after(Duration::from_millis(100));
            EnterFlashingResult::TimedOut
        })
        .await;

        // Probablemente mala idea que ambos acks sean iguales, puede haber problemas de sync
        if ack_received == EnterFlashingResult::AckReceived {
            self.current_offset += FLASH_WRITE_SIZE;
        } else {
            self.revert().await?;
        }

        Ok(())
    }

    async fn revert(&mut self) -> anyhow::Result<()> {
        let frame = CanFrame::Data(
            CanDataFrame::new(REVERT_WORD_MSG_ID, &[])
                .ok_or(anyhow::anyhow!("Could not create can frame"))?,
        );
        self.can.write_frame(&frame).await?;

        let ack_received = async {
            loop {
                let Ok(frame) = self.can.read_frame().await else {
                    continue;
                };

                if frame.id() == ACK_MSG_ID {
                    break EnterFlashingResult::AckReceived;
                }
            }
        }
        .or(async {
            smol::Timer::after(Duration::from_millis(100));
            EnterFlashingResult::TimedOut
        })
        .await;

        if ack_received == EnterFlashingResult::TimedOut {
            anyhow::bail!(
                "Timed out revert of frame writing. Which is a critical error. The bus is shit"
            )
        }
        Ok(())
    }
}
