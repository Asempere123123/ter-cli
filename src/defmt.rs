use std::path::PathBuf;

use anyhow::anyhow;
use probe_rs::{
    Session, SessionConfig,
    rtt::{Rtt, UpChannel},
};

use crate::descriptor::Descriptor;

pub fn attach_defmt(descriptor: &Descriptor, elf_path: PathBuf) -> anyhow::Result<()> {
    log::info!("Attaching defmt logging");
    let session_config = SessionConfig::default();
    let mut session = Session::auto_attach(descriptor.chip_name(), session_config)?;
    let mut core = session.core(0)?;

    let mut rtt = Rtt::attach(&mut core)?;
    let up_channel: &mut UpChannel = rtt
        .up_channels()
        .first_mut()
        .ok_or_else(|| anyhow!("No RTT up channel found"))?;

    let elf_bytes = std::fs::read(elf_path)?;
    let table =
        defmt_decoder::Table::parse(&elf_bytes)?.ok_or(anyhow!("Defmt table not found in ELF"))?;
    let mut decoder = table.new_stream_decoder();

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
