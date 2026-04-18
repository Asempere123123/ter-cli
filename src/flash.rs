use std::{path::Path, time::Duration, u8};

use nix::net::if_::if_nameindex;
use probe_rs::{
    Error, Permissions, Session,
    probe::{DebugProbeError, DebugProbeInfo, ProbeCreationError, list::Lister},
};
use smol::{Timer, process::Command};
use smol_timeout::TimeoutExt;
use socketcan::{
    CanDataFrame, CanFrame, CanInterface, EmbeddedFrame, Id, StandardId, smol::CanSocket,
};

use crate::{descriptor::Descriptor, flash_size};

pub const FLASH_BASE_ADDR: u64 = 0x08000000;

pub fn flash(
    bootloader_path: impl AsRef<Path>,
    app_path: impl AsRef<Path>,
    chip_name: &str,
    bootloader_defmt: bool,
    can: bool,
    descriptor: &Descriptor,
) -> anyhow::Result<Session> {
    log::info!(
        "FLASH_ERASE_SIZE = {}K",
        flash_size::get_first_sector_size(&descriptor)? / 1024
    );
    log::info!("Flashing App");
    let bootloader = std::fs::read(bootloader_path)?;
    let app = std::fs::read(app_path)?;
    if bootloader.len() > flash_size::get_first_sector_size(&descriptor)? as usize {
        anyhow::bail!(
            "Currently built bootloader can't fit on its allocated size. It's size is {}K",
            bootloader.len() / 1024
        );
    }

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
    app.resize(
        app.len().div_ceil(FLASH_SECTOR_WRITE_SIZE) * FLASH_SECTOR_WRITE_SIZE,
        u8::MAX,
    );
    let can_iface = configure_can(descriptor.can_baudrate().ok_or(anyhow::anyhow!(
        "No can baudrate specified. Have you set up the can stack in the ter.toml file?"
    ))?)
    .await?;

    let mut can_socket = CanSocket::open(&can_iface)?;

    wait_bootloader_start(&mut can_socket, descriptor).await?;
    send_flash_info_message(&mut can_socket, app.len() as u32).await?;
    can_send_app_data(&mut can_socket, app).await?;

    log::info!("Flashing success");

    Ok(())
}

async fn wait_bootloader_start(can: &mut CanSocket, descriptor: &Descriptor) -> anyhow::Result<()> {
    let messaging_handle = smol::spawn(async {
        loop {
            log::info!("Waiting to enter bootloader mode");
            Timer::after(Duration::from_millis(1000)).await;
        }
    });

    loop {
        can.write_frame(
            &BeginCanFlashingMessage {
                board_id: descriptor.name_hash(),
            }
            .to_frame(),
        )
        .await?;
        if can_wait_ack(can)
            .timeout(Duration::from_millis(200))
            .await
            .is_some()
        {
            break;
        }
    }

    messaging_handle.cancel().await;
    log::info!("Bootloader mode entered!");

    Ok(())
}

async fn send_flash_info_message(can: &mut CanSocket, app_len: u32) -> anyhow::Result<()> {
    can.write_frame(&BeginFlashInfoMessage { app_len }.to_frame())
        .await?;

    can_wait_ack(can).await;
    log::info!("All info sent, erasing flash");

    can_wait_ack(can).await;
    log::info!("Flash erased, flashing");

    Ok(())
}

async fn can_send_app_data(can: &mut CanSocket, app: Vec<u8>) -> anyhow::Result<()> {
    let mut current_offset = 0;

    while current_offset < app.len() {
        can_send_whole_frame(can, &app, current_offset as u32).await?;

        // Check if all frames where received
        if can_wait_ack(can)
            .timeout(Duration::from_millis(100))
            .await
            .is_none()
        {
            log::info!("Failed to write sector with offset 0x{:X}", current_offset);
            can.write_frame(&RevertSectorMessage.to_frame()).await?;
            can_wait_ack(can).await;
            continue;
        }

        current_offset += FLASH_SECTOR_WRITE_SIZE;
        // Wait to be able to send another frame
        can_wait_ack(can).await;
    }

    log::info!("Finished writing to flash. Sending Done signal");
    can.write_frame(&FlashFinishMessage.to_frame()).await?;
    can_wait_ack(can).await;
    Ok(())
}

async fn can_send_whole_frame(
    can: &mut CanSocket,
    app: &Vec<u8>,
    offset: u32,
) -> anyhow::Result<()> {
    for i in (0..(FLASH_SECTOR_WRITE_SIZE)).step_by(7) {
        let frame = FlashDataMessage {
            index: i as u8,
            data: bytemuck::pod_read_unaligned(
                &app[(offset as usize + i)..(offset as usize + i + 7)],
            ),
        }
        .to_frame();
        while can.write_frame(&frame).await.is_err() {}
    }

    Ok(())
}

async fn can_wait_ack(can: &mut CanSocket) {
    loop {
        let Ok(frame) = can.read_frame().await else {
            continue;
        };

        if AckMessage::try_from_frame(&frame).is_some() {
            break;
        }
    }
}

//// Frame kind types

const FLASH_SECTOR_WRITE_SIZE: usize = 32 * 7;

pub struct BeginCanFlashingMessage {
    board_id: u64,
}

impl BeginCanFlashingMessage {
    const MESSAGE_ID: Id = Id::Standard(unsafe { StandardId::new_unchecked(0x303) });

    #[allow(unused)]
    pub fn try_from_frame(frame: &CanFrame) -> Option<Self> {
        if frame.id() == Self::MESSAGE_ID {
            Some(Self {
                board_id: bytemuck::pod_read_unaligned(frame.data()),
            })
        } else {
            None
        }
    }

    #[allow(unused)]
    pub fn to_frame(self) -> CanDataFrame {
        let Some(frame) = CanDataFrame::new(Self::MESSAGE_ID, bytemuck::bytes_of(&self.board_id))
        else {
            panic!()
        };

        frame
    }
}

pub struct AckMessage;

impl AckMessage {
    const MESSAGE_ID: Id = Id::Standard(unsafe { StandardId::new_unchecked(0x304) });

    #[allow(unused)]
    pub fn try_from_frame(frame: &CanFrame) -> Option<Self> {
        if frame.id() == Self::MESSAGE_ID {
            Some(Self)
        } else {
            None
        }
    }

    #[allow(unused)]
    pub fn to_frame(self) -> CanDataFrame {
        let Some(frame) = CanDataFrame::new(Self::MESSAGE_ID, &[]) else {
            panic!()
        };

        frame
    }
}

#[repr(C)]
#[derive(bytemuck::AnyBitPattern, bytemuck::NoUninit, Clone, Copy)]
pub struct FlashDataMessage {
    index: u8,
    data: [u8; 7],
}

impl FlashDataMessage {
    const MESSAGE_ID: Id = Id::Standard(unsafe { StandardId::new_unchecked(0x305) });

    #[allow(unused)]
    pub fn try_from_frame(frame: &CanFrame) -> Option<Self> {
        if frame.id() == Self::MESSAGE_ID {
            Some(bytemuck::pod_read_unaligned(frame.data()))
        } else {
            None
        }
    }

    #[allow(unused)]
    pub fn to_frame(self) -> CanDataFrame {
        let Some(frame) = CanDataFrame::new(Self::MESSAGE_ID, bytemuck::bytes_of(&self)) else {
            panic!()
        };

        frame
    }
}

#[repr(C)]
#[derive(bytemuck::AnyBitPattern, bytemuck::NoUninit, Clone, Copy)]
pub struct BeginFlashInfoMessage {
    app_len: u32,
}

impl BeginFlashInfoMessage {
    const MESSAGE_ID: Id = Id::Standard(unsafe { StandardId::new_unchecked(0x306) });

    #[allow(unused)]
    pub fn try_from_frame(frame: &CanFrame) -> Option<Self> {
        if frame.id() == Self::MESSAGE_ID {
            Some(bytemuck::pod_read_unaligned(frame.data()))
        } else {
            None
        }
    }

    #[allow(unused)]
    pub fn to_frame(self) -> CanDataFrame {
        let Some(frame) = CanDataFrame::new(Self::MESSAGE_ID, bytemuck::bytes_of(&self)) else {
            panic!()
        };

        frame
    }
}

pub struct FlashFinishMessage;

impl FlashFinishMessage {
    const MESSAGE_ID: Id = Id::Standard(unsafe { StandardId::new_unchecked(0x307) });

    #[allow(unused)]
    pub fn try_from_frame(frame: &CanFrame) -> Option<Self> {
        if frame.id() == Self::MESSAGE_ID {
            Some(Self)
        } else {
            None
        }
    }

    #[allow(unused)]
    pub fn to_frame(self) -> CanDataFrame {
        let Some(frame) = CanDataFrame::new(Self::MESSAGE_ID, &[]) else {
            panic!()
        };

        frame
    }
}

pub struct RevertSectorMessage;

impl RevertSectorMessage {
    const MESSAGE_ID: Id = Id::Standard(unsafe { StandardId::new_unchecked(0x308) });

    #[allow(unused)]
    pub fn try_from_frame(frame: &CanFrame) -> Option<Self> {
        if frame.id() == Self::MESSAGE_ID {
            Some(Self)
        } else {
            None
        }
    }

    #[allow(unused)]
    pub fn to_frame(self) -> CanDataFrame {
        let Some(frame) = CanDataFrame::new(Self::MESSAGE_ID, &[]) else {
            panic!()
        };

        frame
    }
}
