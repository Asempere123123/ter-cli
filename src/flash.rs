use std::path::Path;

use probe_rs::{
    Error, Permissions, Session,
    probe::{DebugProbeError, DebugProbeInfo, ProbeCreationError, list::Lister},
};

const BOOTLOADER_SIZE: u64 = 16 * 1024;

pub fn flash(
    bootloader_path: impl AsRef<Path>,
    app_path: impl AsRef<Path>,
    chip_name: &str,
    bootloader_defmt: bool,
) -> anyhow::Result<Session> {
    log::info!("Flashing App");
    let bootloader = std::fs::read(bootloader_path)?;
    let app = std::fs::read(app_path)?;

    let mut session = get_session(chip_name)?;
    let mut loader = session.target().flash_loader();

    loader.add_data(0x08000000, &bootloader)?;
    if !bootloader_defmt {
        loader.add_data(0x08000000 + BOOTLOADER_SIZE, &app)?;
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
