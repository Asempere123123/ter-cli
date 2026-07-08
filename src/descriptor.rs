use probe_rs::{CoreType, config::Registry};
use serde::{Deserialize, Serialize};
use std::{
    hash::{DefaultHasher, Hash, Hasher},
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize, Clone)]
pub struct Descriptor {
    project_name: String,
    chip_name: String,
    bin_path: Option<PathBuf>,
    elf_path: Option<PathBuf>,
    build_command: Option<String>,
    hse: Option<String>,
    can: Option<String>,
    can_tx_int_name: Option<String>,
    can_rx0_int_name: Option<String>,
    can_rx1_int_name: Option<String>,
    can_sce_int_name: Option<String>,
    can_tx: Option<String>,
    can_rx: Option<String>,
    can2: Option<String>,
    can2_tx: Option<String>,
    can2_rx: Option<String>,
    can_baudrate: Option<String>,
    fdcan: Option<String>,
    fdcan_rx: Option<String>,
    fdcan_tx: Option<String>,
    fdcan_int0_name: Option<String>,
    fdcan_int1_name: Option<String>,
    smps_power: Option<bool>,
    string_rtt: Option<bool>,
    flash_size: Option<u64>,
}

impl Descriptor {
    pub fn from_path(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let desc: Descriptor = toml::from_str(&std::fs::read_to_string(path)?)?;

        let has_can = match (
            desc.can.is_some(),
            desc.can_tx.is_some(),
            desc.can_rx.is_some(),
        ) {
            (true, true, true) => true,
            (false, false, false) => false,
            _ => anyhow::bail!(
                r#"Either all of "can", "can_tx", "can_rx" must be defined or none in ter.toml"#
            ),
        };

        let has_can2 = match (
            desc.can2.is_some(),
            desc.can2_tx.is_some(),
            desc.can2_rx.is_some(),
        ) {
            (true, true, true) => true,
            (false, false, false) => false,
            _ => anyhow::bail!(
                r#"Either all of "can2", "can2_tx", "can2_rx" must be defined or none in ter.toml"#
            ),
        };

        let has_fdcan = match (
            desc.fdcan.is_some(),
            desc.fdcan_tx.is_some(),
            desc.fdcan_rx.is_some(),
        ) {
            (true, true, true) => true,
            (false, false, false) => false,
            _ => anyhow::bail!(
                r#"Either all of "fdcan", "fdcan_tx", "fdcan_rx" must be defined or none in ter.toml"#
            ),
        };

        if has_can2 && !has_can {
            anyhow::bail!(
                "CAN2 is defined, but the primary 'can' interface is not. CAN2 requires CAN to be enabled."
            );
        }

        let needs_baudrate = has_can || has_can2 || has_fdcan;

        match (needs_baudrate, desc.can_baudrate.is_some()) {
            (true, false) => {
                anyhow::bail!("can_baudrate must be defined when a CAN/FDCAN interface is enabled")
            }
            (false, true) => {
                anyhow::bail!("can_baudrate is defined, but no CAN/FDCAN interfaces are enabled")
            }
            _ => {}
        }

        let has_can_ints = desc.can_tx_int_name.is_some()
            || desc.can_rx0_int_name.is_some()
            || desc.can_rx1_int_name.is_some()
            || desc.can_sce_int_name.is_some();

        if has_can_ints && !has_can {
            anyhow::bail!(
                "Classic CAN interrupts are defined, but the 'can' interface is not enabled"
            );
        }

        let has_fdcan_ints = desc.fdcan_int0_name.is_some() || desc.fdcan_int1_name.is_some();

        if has_fdcan_ints && !has_fdcan {
            anyhow::bail!("FDCAN interrupts are defined, but the 'fdcan' interface is not enabled");
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

    pub fn flash_size(&self) -> &Option<u64> {
        &self.flash_size
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
                "can-tx-int-name={}",
                self.can_tx_int_name
                    .clone()
                    .or(self.can.as_ref().map(|can| format!("{}_TX", can.as_str())))
                    .unwrap_or(String::from("NONE"))
            ),
            slash_d.clone(),
            format!(
                "can-rx0-int-name={}",
                self.can_rx0_int_name
                    .clone()
                    .or(self.can.as_ref().map(|can| format!("{}_RX0", can.as_str())))
                    .unwrap_or(String::from("NONE"))
            ),
            slash_d.clone(),
            format!(
                "can-rx1-int-name={}",
                self.can_rx1_int_name
                    .clone()
                    .or(self.can.as_ref().map(|can| format!("{}_RX1", can.as_str())))
                    .unwrap_or(String::from("NONE"))
            ),
            slash_d.clone(),
            format!(
                "can-sce-int-name={}",
                self.can_sce_int_name
                    .clone()
                    .or(self.can.as_ref().map(|can| format!("{}_SCE", can.as_str())))
                    .unwrap_or(String::from("NONE"))
            ),
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
            format!("can2={}", self.can2.clone().unwrap_or(String::from("NONE"))),
            slash_d.clone(),
            format!(
                "can2-tx={}",
                self.can2_tx.clone().unwrap_or(String::from("NONE"))
            ),
            slash_d.clone(),
            format!(
                "can2-rx={}",
                self.can2_rx.clone().unwrap_or(String::from("NONE"))
            ),
            slash_d.clone(),
            format!(
                "can-baudrate={}",
                self.can_baudrate.clone().unwrap_or(String::from("NONE"))
            ),
            slash_d.clone(),
            format!(
                "fdcan={}",
                self.fdcan.clone().unwrap_or(String::from("NONE"))
            ),
            slash_d.clone(),
            format!(
                "fdcan-rx={}",
                self.fdcan_rx.clone().unwrap_or(String::from("NONE"))
            ),
            slash_d.clone(),
            format!(
                "fdcan-tx={}",
                self.fdcan_tx.clone().unwrap_or(String::from("NONE"))
            ),
            slash_d.clone(),
            format!(
                "fdcan-it0-int-name={}",
                self.fdcan_int0_name
                    .clone()
                    .or(self
                        .fdcan
                        .as_ref()
                        .map(|can| format!("{}_IT0", can.as_str())))
                    .unwrap_or(String::from("NONE"))
            ),
            slash_d.clone(),
            format!(
                "fdcan-it1-int-name={}",
                self.fdcan_int1_name
                    .clone()
                    .or(self
                        .fdcan
                        .as_ref()
                        .map(|can| format!("{}_IT1", can.as_str())))
                    .unwrap_or(String::from("NONE"))
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
        if self.fdcan.is_some() {
            args.push("fdcan");
        }
        if let Some(smps_power) = self.smps_power
            && smps_power
        {
            args.push("smps_power");
        }

        args.into_iter()
    }

    pub fn get_identity(&self) -> anyhow::Result<String> {
        let digest = md5::compute(serde_jcs::to_string(&DescriptorJson::from(self))?);
        Ok(format!("{:x}", digest))
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
    can_tx_int_name: &'a Option<String>,
    can_rx0_int_name: &'a Option<String>,
    can_rx1_int_name: &'a Option<String>,
    can_sce_int_name: &'a Option<String>,
    can_tx: &'a Option<String>,
    can_rx: &'a Option<String>,
    can2: &'a Option<String>,
    can2_tx: &'a Option<String>,
    can2_rx: &'a Option<String>,
    can_baudrate: &'a Option<String>,
    fdcan: &'a Option<String>,
    fdcan_rx: &'a Option<String>,
    fdcan_tx: &'a Option<String>,
    fdcan_int0_name: &'a Option<String>,
    fdcan_int1_name: &'a Option<String>,
    smps_power: &'a Option<bool>,
    flash_size: &'a Option<u64>,
}

impl<'a> From<&'a Descriptor> for DescriptorJson<'a> {
    fn from(d: &'a Descriptor) -> Self {
        Self {
            project_name: &d.project_name,
            hse: &d.hse,
            can: &d.can,
            can_tx_int_name: &d.can_tx_int_name,
            can_rx0_int_name: &d.can_rx0_int_name,
            can_rx1_int_name: &d.can_rx1_int_name,
            can_sce_int_name: &d.can_sce_int_name,
            can_tx: &d.can_tx,
            can_rx: &d.can_rx,
            can2: &d.can2,
            can2_tx: &d.can2_tx,
            can2_rx: &d.can2_rx,
            can_baudrate: &d.can_baudrate,
            fdcan: &d.fdcan,
            fdcan_rx: &d.fdcan_rx,
            fdcan_tx: &d.fdcan_tx,
            fdcan_int0_name: &d.fdcan_int0_name,
            fdcan_int1_name: &d.fdcan_int1_name,
            smps_power: &d.smps_power,
            flash_size: &d.flash_size,
        }
    }
}
