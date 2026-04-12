use crate::{descriptor::Descriptor, flash::FLASH_BASE_ADDR};
use cached::proc_macro::cached;
use serde_json::Value;

const MIN_BOOTLOADER_SIZE: u64 = 16 * 1024;

#[cached(
    key = "String",
    convert = r#"{ desc.chip_name().to_owned() }"#,
    result = true
)]
pub fn get_first_sector_size(desc: &Descriptor) -> anyhow::Result<u64> {
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

    first_flash["settings"]["erase_size"]
        .as_u64()
        .map(|flash_size| flash_size.max(MIN_BOOTLOADER_SIZE))
        .ok_or(anyhow::anyhow!("First sector size not found"))
}
