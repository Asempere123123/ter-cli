use std::{
    hash::{DefaultHasher, Hash, Hasher},
    path::{Path, PathBuf},
};

use heck::ToKebabCase;
use probe_rs::{CoreType, config::Registry};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Descriptor {
    project_name: String,
    chip_name: String,
    bin_path: Option<PathBuf>,
    elf_path: Option<PathBuf>,
    build_command: Option<String>,
    hse: Option<String>,
    can: Option<String>,
    can_tx: Option<String>,
    can_rx: Option<String>,
    can2: Option<String>,
    can2_tx: Option<String>,
    can2_rx: Option<String>,
    can_baudrate: Option<String>,
    string_rtt: Option<bool>,
}

impl Descriptor {
    pub fn from_path(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let desc: Descriptor = toml::from_str(&std::fs::read_to_string(path)?)?;

        if !((desc.can.is_some()
            && desc.can_tx.is_some()
            && desc.can_rx.is_some()
            && desc.can_baudrate.is_some())
            || (desc.can.is_none()
                && desc.can_tx.is_none()
                && desc.can_rx.is_none()
                && desc.can_baudrate.is_none()))
        {
            anyhow::bail!(
                r#"Either all "can", "can_tx", "can_rx", "can_baudrate" must be defined or none in ter.toml"#
            );
        }

        if !((desc.can2.is_some()
            && desc.can2_tx.is_some()
            && desc.can2_rx.is_some()
            && desc.can_baudrate.is_some())
            || (desc.can2.is_none() && desc.can2_tx.is_none() && desc.can2_rx.is_none()))
        {
            anyhow::bail!(
                r#"Either all "can2", "can2_tx", "can2_rx", "can_baudrate" must be defined or none in ter.toml"#
            );
        }

        Ok(desc)
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

    pub fn uses_string_rtt(&self) -> Option<bool> {
        self.string_rtt
    }

    pub fn name_hash(&self) -> u64 {
        let mut chip_hasher = DefaultHasher::new();
        self.project_name.hash(&mut chip_hasher);
        let project_hash: u64 = chip_hasher.finish();
        project_hash
    }

    pub fn get_generate_args(&self) -> impl Iterator<Item = String> {
        let slash_d = String::from("-d");
        let project_hash = self.name_hash();

        [
            slash_d.clone(),
            format!("board-hash={}", project_hash),
            slash_d.clone(),
            format!("hse-freq={}", self.hse.clone().unwrap_or(String::from("0"))),
            slash_d.clone(),
            format!("can={}", self.can.clone().unwrap_or(String::from("NONE"))),
            slash_d.clone(),
            format!(
                "can-tx={}",
                self.can_tx.clone().unwrap_or(String::from("NONE"))
            ),
            slash_d.clone(),
            format!(
                "can-rx={}",
                self.can_rx.clone().unwrap_or(String::from("NONE"))
            ),
            slash_d.clone(),
            format!("can2={}", self.can.clone().unwrap_or(String::from("NONE"))),
            slash_d.clone(),
            format!(
                "can2-tx={}",
                self.can_tx.clone().unwrap_or(String::from("NONE"))
            ),
            slash_d.clone(),
            format!(
                "can2-rx={}",
                self.can_rx.clone().unwrap_or(String::from("NONE"))
            ),
            slash_d.clone(),
            format!(
                "can-baudrate={}",
                self.can_baudrate.clone().unwrap_or(String::from("NONE"))
            ),
        ]
        .into_iter()
    }

    pub fn get_objcopy_features(&self) -> impl Iterator<Item = &str> {
        let mut args = Vec::new();

        if self.hse.is_some() {
            args.push("hse");
        }
        if self.can.is_some() {
            args.push("can");
        }
        if self.can2.is_some() {
            args.push("can2");
        }

        args.into_iter()
    }

    pub fn get_identity_json(&self) -> anyhow::Result<String> {
        Ok(serde_jcs::to_string(&DescriptorJson::from(self))?.to_kebab_case())
    }

    pub fn can_baudrate(&self) -> Option<u32> {
        self.can_baudrate.as_deref().and_then(|s| s.parse().ok())
    }
}

#[derive(Serialize, Debug)]
pub struct DescriptorJson<'a> {
    project_name: &'a String,
    hse: &'a Option<String>,
    can: &'a Option<String>,
    can_tx: &'a Option<String>,
    can_rx: &'a Option<String>,
    can_baudrate: &'a Option<String>,
}

impl<'a> From<&'a Descriptor> for DescriptorJson<'a> {
    fn from(d: &'a Descriptor) -> Self {
        Self {
            project_name: &d.project_name,
            hse: &d.hse,
            can: &d.can,
            can_tx: &d.can_tx,
            can_rx: &d.can_rx,
            can_baudrate: &d.can_baudrate,
        }
    }
}
