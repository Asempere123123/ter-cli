use std::path::Path;

use nix::net::if_::if_nameindex;
use probe_rs::{
    Error, Permissions, Session,
    probe::{DebugProbeError, DebugProbeInfo, ProbeCreationError, list::Lister},
};
use smol::process::Command;
use socketcan::{
    CanDataFrame, CanFrame, CanInterface, EmbeddedFrame, Id, StandardId, smol::CanSocket,
};

use crate::{descriptor::Descriptor, flash_size};

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

    loader.add_data(0x08000000, &bootloader)?;
    if !bootloader_defmt {
        loader.add_data(
            0x08000000 + flash_size::get_first_sector_size(&descriptor)?,
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

async fn flash_can(descriptor: &Descriptor, app: Vec<u8>) -> anyhow::Result<()> {
    let can_iface = configure_can(descriptor.can_baudrate().ok_or(anyhow::anyhow!(
        "No can baudrate specified. Have you set up the can stack in the ter.toml file?"
    ))?)
    .await?;

    let can_socket = CanSocket::open(&can_iface)?;

    let data_frame = CanDataFrame::new(
        Id::Standard(StandardId::new(1).ok_or(anyhow::anyhow!("Invalid standard id created"))?),
        &[1],
    )
    .ok_or(anyhow::anyhow!("Invalid dataframe produced"))?;
    can_socket
        .write_frame(&CanFrame::Data(data_frame))
        .await
        .unwrap();

    Ok(())
}
