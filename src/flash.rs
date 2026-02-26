use std::path::Path;

use probe_rs::{Session, SessionConfig};

const BOOTLOADER_SIZE: u64 = 16 * 1024;

pub fn flash(
    bootloader_path: impl AsRef<Path>,
    app_path: impl AsRef<Path>,
    chip_name: &str,
    bootloader_defmt: bool,
) -> anyhow::Result<()> {
    log::info!("Flashing App");
    let bootloader = std::fs::read(bootloader_path)?;
    let app = std::fs::read(app_path)?;

    let session_config = SessionConfig::default();
    let mut session = Session::auto_attach(chip_name, session_config)?;

    let mut loader = session.target().flash_loader();

    loader.add_data(0x08000000, &bootloader)?;
    if !bootloader_defmt {
        loader.add_data(0x08000000 + BOOTLOADER_SIZE, &app)?;
    }
    loader.commit(&mut session, Default::default())?;

    log::info!("Flashing Complete. Reseting app");
    let mut core = session.core(0)?;
    core.reset()?;
    Ok(())
}
