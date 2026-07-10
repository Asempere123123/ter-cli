use anyhow::anyhow;
use log::info;
use object::{Object, ObjectSymbol};
use probe_rs::{
    Core, Session,
    rtt::{DownChannel, Rtt, UpChannel},
};
use std::{fs, path::Path};

/// PROTOCOL:
/// We send 0xD
/// App responds 0xDD
/// App sends LE bytes of erase size as u32
/// App sends LE bytes of write size as u32
/// We send LE bytes of app len as u32. Rounding for erase size
/// App sends 0xA When finished
/// We send write size blocks of app, untill we sent everything is sent
///     After each block is sent we wait for write ack: 0xB
/// When finished, we wait for the app to return 0xC to ack it finished writing

pub async fn flash_external(
    session: &mut Session,
    bootloader_elf_path: impl AsRef<Path>,
    app_path: impl AsRef<Path>,
) -> anyhow::Result<()> {
    log::info!("Flashing External flash");
    let mut app = std::fs::read(app_path)?;
    let mut core = session.core(0)?;

    let location = find_rtt_control_block(bootloader_elf_path)?;
    let rtt = Rtt::attach_at(&mut core, location)?;

    let mut down_channel = rtt
        .down_channels
        .into_iter()
        .filter(|channel| channel.name() == Some("commands"))
        .next()
        .ok_or_else(|| anyhow!("No RTT down channel found"))?;
    let mut up_channel = rtt
        .up_channels
        .into_iter()
        .filter(|channel| channel.name() == Some("commands"))
        .next()
        .ok_or_else(|| anyhow!("No RTT up channel found"))?;

    // Start Flash Writes
    down_channel.write(&mut core, &[0xD])?;
    info!("Waiting for bootloader to respond to external flash write request");
    let mut buf = [0];
    read_buf(&mut up_channel, &mut core, &mut buf).await?;
    if buf[0] != 0xDD {
        anyhow::bail!("App did not accet the request");
    }
    let mut erase_size = [0; 4];
    read_buf(&mut up_channel, &mut core, &mut erase_size).await?;
    let erase_size = u32::from_le_bytes(erase_size);
    let mut write_size = [0; 4];
    read_buf(&mut up_channel, &mut core, &mut write_size).await?;
    let write_size = u32::from_le_bytes(write_size);
    info!("Bootloader accepted external flash write request!");

    down_channel.write(
        &mut core,
        &(app.len() as u32)
            .next_multiple_of(erase_size)
            .to_le_bytes(),
    )?;

    let mut buf = [0];
    read_buf(&mut up_channel, &mut core, &mut buf).await?;
    if buf[0] != 0xA {
        anyhow::bail!("Expected erase finished response, got {}", buf[0]);
    }
    info!("Finished erasing external flash");

    app.extend(std::iter::repeat_n(
        u8::MAX,
        app.len().next_multiple_of(write_size as usize) - app.len(),
    ));
    for i in (0..app.len()).step_by(write_size as usize) {
        write_buf(
            &mut down_channel,
            &mut core,
            &app[i..(i + write_size as usize)],
        )
        .await?;

        let mut buf = [0];
        read_buf(&mut up_channel, &mut core, &mut buf).await?;
        if buf[0] != 0xB {
            anyhow::bail!("Expected sector write ACK, got: {}", buf[0]);
        }
    }
    let mut buf = [0];
    read_buf(&mut up_channel, &mut core, &mut buf).await?;
    if buf[0] != 0xC {
        anyhow::bail!("Expected finish ACK, got: {}", buf[0]);
    }

    info!("Finished flashing external flash");
    Ok(())
}

async fn read_buf(
    up_channel: &mut UpChannel,
    core: &mut Core<'_>,
    buf: &mut [u8],
) -> anyhow::Result<()> {
    let mut amount_read = 0;
    while amount_read != buf.len() {
        amount_read += up_channel.read(core, &mut buf[amount_read..])?;
        smol::future::yield_now().await;
    }
    Ok(())
}

async fn write_buf(
    down_channel: &mut DownChannel,
    core: &mut Core<'_>,
    buf: &[u8],
) -> anyhow::Result<()> {
    let mut amount_written = 0;
    while amount_written < buf.len() {
        let written = down_channel.write(core, &buf[amount_written..])?;
        amount_written += written;
        smol::future::yield_now().await;
    }
    Ok(())
}

fn find_rtt_control_block(bootloader_elf_path: impl AsRef<Path>) -> anyhow::Result<u64> {
    let bin_data = fs::read(bootloader_elf_path)?;
    let file = object::File::parse(&*bin_data)?;

    for symbol in file.symbols() {
        if let Ok(name) = symbol.name() {
            if name == "_SEGGER_RTT" {
                return Ok(symbol.address());
            }
        }
    }

    anyhow::bail!("No RTT control block found")
}
