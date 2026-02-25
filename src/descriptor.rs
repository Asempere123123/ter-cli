use std::path::{Path, PathBuf};

use probe_rs::{CoreType, config::Registry};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Descriptor {
    chip_name: String,
    bin_path: Option<PathBuf>,
    elf_path: Option<PathBuf>,
    build_command: Option<String>,
}

impl Descriptor {
    pub fn from_path(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        Ok(toml::from_str(&std::fs::read_to_string(path)?)?)
    }

    pub fn chip_name(&self) -> &str {
        &self.chip_name
    }

    pub fn chip_hal_name(&self) -> String {
        // Si tiene cosas raras al final no hacen falta
        self.chip_name[..11].to_lowercase()
    }

    pub fn chip_arch_name(&self) -> anyhow::Result<String> {
        let entry = Registry::from_builtin_families().get_target_by_name(&self.chip_name)?;

        if let Some(core) = entry.cores.first() {
            let target_triple = match core.core_type {
                CoreType::Armv6m => "thumbv6m-none-eabi",
                CoreType::Armv7m => "thumbv7m-none-eabi",
                CoreType::Armv7em => "thumbv7em-none-eabi",
                CoreType::Armv8m => "thumbv8m.main-none-eabi",
                CoreType::Armv7a => "armv7a-none-eabi",
                CoreType::Armv8a => "aarch64-none-elf",
                _ => return Err(anyhow::anyhow!("Unknown CoreType mapping")),
            };

            Ok(target_triple.to_owned())
        } else {
            Err(anyhow::anyhow!("No cores found for this chip"))
        }
    }

    pub fn bin_path(&self) -> &Option<PathBuf> {
        &self.bin_path
    }

    pub fn elf_path(&self) -> &Option<PathBuf> {
        &self.elf_path
    }

    pub fn build_command(&self) -> &Option<String> {
        &self.build_command
    }
}
