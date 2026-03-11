use std::path::Path;

use probe_rs::{
    Error, Permissions, Session,
    probe::{DebugProbeError, ProbeCreationError, list::Lister},
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

    let lister = Lister::new();
    let probes = lister.list_all();
    let probe = probes
        .first()
        .ok_or(Error::Probe(DebugProbeError::ProbeCouldNotBeCreated(
            ProbeCreationError::NotFound,
        )))?
        .open()?;
    let mut session = probe.attach_under_reset(chip_name, Permissions::default())?;

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
