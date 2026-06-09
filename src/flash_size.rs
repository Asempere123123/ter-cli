use crate::{descriptor::Descriptor, flash::FLASH_BASE_ADDR};
use cached::proc_macro::cached;
use serde_json::Value;

const DEFAULT_BOOTLOADER_SIZE: u64 = 16;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/v", env!("CARGO_PKG_VERSION"),);

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

pub fn get_chip_names() -> anyhow::Result<Vec<String>> {
    let res = reqwest::blocking::ClientBuilder::new()
        .user_agent(APP_USER_AGENT)
        .build()?
        .get("https://api.github.com/repos/embassy-rs/stm32-data-generated/contents/data/chips")
        .send()?;
    let json: Value = serde_json::from_str(&res.text()?)?;

    Ok(json
        .as_array()
        .ok_or(anyhow::anyhow!("Invalid chip list received"))?
        .iter()
        .flat_map(|chip| {
            chip["name"]
                .as_str()
                .map(|name| {
                    name.strip_suffix(".json")
                        .map(|name_stripped| name_stripped.to_owned())
                })
                .flatten()
        })
        .collect())
}

pub fn get_chip_sizes(chip_name: &str) -> anyhow::Result<ChipSizes> {
    let res = reqwest::blocking::get(format!(
        "https://raw.githubusercontent.com/embassy-rs/stm32-data-generated/refs/heads/main/data/chips/{}.json",
        chip_name
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

    let first_flash_erase_size = first_flash["settings"]["erase_size"]
        .as_u64()
        .ok_or(anyhow::anyhow!("First sector erase size not found"))?
        .max(DEFAULT_BOOTLOADER_SIZE * 1024);

    let flash_size = json["memory"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|memory| memory.as_array())
        .flatten()
        .filter(|memory| {
            memory["kind"].as_str() == Some("flash")
                && memory["name"]
                    .as_str()
                    .map(|name| name.contains("OTP"))
                    .is_some_and(|is_otp| !is_otp)
        })
        .filter_map(|flash| flash["size"].as_u64())
        .sum();

    let ram_size = json["memory"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|memory| memory.as_array())
        .flatten()
        .filter(|memory| memory["kind"].as_str() == Some("ram")) // Desconozco si existe algun tipo de ram exotico que no nos interese, para cosas normales creo q nos da igual
        .filter_map(|flash| flash["size"].as_u64())
        .sum();

    let peripherals = json["cores"]
        .as_array()
        .ok_or(anyhow::anyhow!("Chip has no cores"))?
        .first()
        .ok_or(anyhow::anyhow!("Chip has no cores"))?["peripherals"]
        .as_array()
        .ok_or(anyhow::anyhow!("Core has no peripherals"))?
        .clone();

    Ok(ChipSizes {
        erase_size: first_flash_erase_size,
        flash_size,
        ram_size,
        peripherals,
    })
}

pub struct ChipSizes {
    pub erase_size: u64,
    pub flash_size: u64,
    pub ram_size: u64,
    pub peripherals: Vec<Value>,
}
