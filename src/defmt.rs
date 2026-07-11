use anyhow::anyhow;
use probe_rs::{
    Session,
    rtt::{Rtt, UpChannel},
};
use std::{io::Write, path::PathBuf};

use crate::flash_external_flash::find_rtt_control_block;

pub fn attach_defmt(mut session: Session, elf_path: PathBuf) -> anyhow::Result<()> {
    log::info!("Attaching defmt logging");
    let mut core = session.core(0)?;

    let elf_bytes = std::fs::read(&elf_path)?;
    let table =
        defmt_decoder::Table::parse(&elf_bytes)?.ok_or(anyhow!("Defmt table not found in ELF"))?;
    let mut decoder = table.new_stream_decoder();

    let location = find_rtt_control_block(elf_path).ok();
    let mut rtt = match location {
        Some(location) => Rtt::attach_at(&mut core, location).ok().or_else(|| {
            log::warn!("Could not detect RTT controll block specified in elf file. Beware that this might still detect the bootloader's Rtt block");
            Rtt::attach(&mut core).ok()
        }),
        None => {
            log::info!("Could not find RTT location in elf file. Is debug disabled?");
            Rtt::attach(&mut core).ok()
        }
    }
    .ok_or(anyhow::anyhow!("Could not find RTT controll block"))?;

    let up_channel: &mut UpChannel = rtt
        .up_channels()
        .first_mut()
        .ok_or_else(|| anyhow!("No RTT up channel found"))?;

    log::info!("Attached defmt logging");

    let mut buffer = [0u8; 1024 * 4];
    loop {
        let read = up_channel.read(&mut core, &mut buffer)?;
        if read > 0 {
            decoder.received(&buffer[..read]);

            while let Ok(frame) = decoder.decode() {
                println!("{}", frame.display(true));
            }
        }
    }
}

pub fn attach_string_rtt(mut session: Session) -> anyhow::Result<()> {
    log::info!("Attaching RTT logging");
    let mut core = session.core(0)?;

    let mut rtt = Rtt::attach(&mut core)?;
    let up_channel: &mut UpChannel = rtt
        .up_channels()
        .first_mut()
        .ok_or_else(|| anyhow!("No RTT up channel found"))?;

    let mut buffer = [0u8; 1024 * 4];
    loop {
        let read = up_channel.read(&mut core, &mut buffer)?;
        if read > 0 {
            let message = String::from_utf8_lossy(&buffer[..read]);
            print!("{}", message);
            std::io::stdout().flush()?;
        }
    }
}
