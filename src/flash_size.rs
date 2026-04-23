use crate::{descriptor::Descriptor, flash::FLASH_BASE_ADDR};
use cached::proc_macro::cached;
use serde_json::Value;

const DEFAULT_BOOTLOADER_SIZE: u64 = 16;

#[cached(
    key = "String",
    convert = r#"{ desc.chip_name().to_owned() }"#,
    result = true
)]
pub fn get_first_sector_erase_and_write_size(desc: &Descriptor) -> anyhow::Result<Sizes> {
    let res = reqwest::blocking::get(format!(
        "https://raw.githubusercontent.com/embassy-rs/stm32-data-generated/refs/heads/main/data/chips/{}.json",
        desc.chip_name()
    ))?;
    let json: Value = serde_json::from_str(&res.text()?)?;

    let first_flash = json["memory"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|memory| memory.as_array())
        .flatten()
        .filter(|memory| {
            memory["kind"].as_str() == Some("flash")
                && memory["address"].as_u64() == Some(FLASH_BASE_ADDR)
        })
        .next()
        .ok_or(anyhow::anyhow!("No flash sector found for this chip"))?;

    let first_flash_size = first_flash["settings"]["erase_size"]
        .as_u64()
        .ok_or(anyhow::anyhow!("First sector size not found"))?;

    let required_bootloader_size = desc.flash_size().unwrap_or(DEFAULT_BOOTLOADER_SIZE);

    if (required_bootloader_size * 1024) % first_flash_size != 0 {
        anyhow::bail!(
            "Specified bootloader size is not a multiple of the erase size, which is {}K",
            first_flash_size / 1024
        );
    }

    let Some(write_size) = first_flash["settings"]["write_size"]
        .as_u64()
        .map(|size| size as u8)
    else {
        anyhow::bail!("Write size for current chip not found");
    };

    Ok(Sizes {
        erase_size: required_bootloader_size * 1024,
        write_size,
    })
}

#[derive(Clone, Copy)]
pub struct Sizes {
    pub erase_size: u64,
    pub write_size: u8,
}
