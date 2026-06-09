use crate::{
    flash::{DEFAULT_SECTOR_SIZE, FLASH_BASE_ADDR},
    flash_size::{ChipSizes, get_chip_names, get_chip_sizes},
};
use std::{
    fmt::Display,
    process::{Command, Stdio},
};

// AdinAck/cargo-embassy/chip/target.rs
const TARGETS: &[(&str, Target)] = &[
    ("STM32C0", Target::Thumbv6),
    ("STM32F0", Target::Thumbv6),
    ("STM32F1", Target::Thumbv7),
    ("STM32F2", Target::Thumbv7),
    ("STM32F3", Target::Thumbv7e),
    ("STM32F4", Target::Thumbv7e),
    ("STM32F7", Target::Thumbv7e),
    ("STM32G0", Target::Thumbv6),
    ("STM32G4", Target::Thumbv7f),
    ("STM32H5", Target::Thumbv8),
    ("STM32H7", Target::Thumbv7e),
    ("STM32L0", Target::Thumbv6),
    ("STM32L1", Target::Thumbv7),
    ("STM32L4", Target::Thumbv7e),
    ("STM32L5", Target::Thumbv8),
    ("STM32U5", Target::Thumbv8),
    ("STM32WB", Target::Thumbv7e),
    ("STM32WBA", Target::Thumbv8),
    ("STM32WL", Target::Thumbv7e),
];

pub fn create_new() -> anyhow::Result<()> {
    // Get data
    let project_name = dialoguer::Input::<String>::new()
        .with_prompt("Project Name")
        .interact_text()?;

    let chips = get_chip_names()?;
    let selection = dialoguer::FuzzySelect::new()
        .with_prompt("Select your chip")
        .items(&chips)
        .interact()?;
    let chip = chips[selection].clone();

    let sizes = get_chip_sizes(&chip)?;
    log::info!("{}", chip);

    let arch = TARGETS
        .iter()
        .filter(|target| chip.contains(target.0))
        .map(|target| target.1)
        .next()
        .ok_or(anyhow::anyhow!("Chip arch not found"))?;

    // Generate from template
    let mut extra_args = String::new();

    if sizes.erase_size != DEFAULT_SECTOR_SIZE as u64 * 1024 {
        extra_args += &format!("flash_size = {}\n", sizes.erase_size / 1024);
    }

    interrogate_hse(&mut extra_args)?;
    interrogate_can_flashing(&mut extra_args, &sizes)?;

    let mut cargo_generate_cmd = Command::new("cargo");
    cargo_generate_cmd
        .args([
            "generate",
            "gh:Asempere123123/ter-template",
            "--name",
            &project_name,
            "-d",
            &format!("flash-begin=0x{:X}", FLASH_BASE_ADDR + sizes.erase_size),
            "-d",
            &format!(
                "flash-len={}K",
                (sizes.flash_size - sizes.erase_size) / 1024
            ),
            "-d",
            &format!("ram-len={}K", sizes.ram_size / 1024),
            "-d",
            &format!("chip-name={}", chip),
            "-d",
            &format!("chip-arch={}", arch),
        ])
        .env("CARGO_GENERATE_VALUE_EXTRA-ARGS", extra_args);
    let status = cargo_generate_cmd
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    if !status.success() {
        anyhow::bail!("Cargo generate failed with status: {}", status);
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub enum Target {
    Thumbv6,
    Thumbv7,
    Thumbv7e,
    Thumbv7f,
    Thumbv8,
}

impl Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Thumbv6 => "thumbv6m-none-eabi",
            Self::Thumbv7 => "thumbv7m-none-eabi",
            Self::Thumbv7e => "thumbv7em-none-eabi",
            Self::Thumbv7f => "thumbv7em-none-eabihf",
            Self::Thumbv8 => "thumbv8m.main-none-eabihf",
        })
    }
}

fn interrogate_hse(extra_args: &mut String) -> anyhow::Result<()> {
    let response = dialoguer::Select::new()
        .with_prompt("Add HSE support")
        .items(&["Yes", "NO"])
        .interact()?;

    if response == 1 {
        return Ok(());
    }

    let speed = dialoguer::Input::<u64>::new()
        .with_prompt("HSE speed")
        .interact_text()?;

    extra_args.push_str(&format!("hse = \"{}\"\n", speed));

    Ok(())
}

fn interrogate_can_flashing(extra_args: &mut String, sizes: &ChipSizes) -> anyhow::Result<()> {
    let response = dialoguer::Select::new()
        .with_prompt("Add CAN flashing support")
        .items(&["No", "In CAN Master Peripheral", "In CAN Slave Peripheral"])
        .interact()?;

    if response == 0 {
        return Ok(());
    }
    if response == 1 {
        return interrogate_can_master_flashing(extra_args, sizes);
    }
    if response == 2 {
        return interrogate_can_slave_flashing(extra_args, sizes);
    }

    Ok(())
}

fn interrogate_can_master_flashing(
    extra_args: &mut String,
    sizes: &ChipSizes,
) -> anyhow::Result<()> {
    // Select CAN Peri
    let can_peris = sizes
        .peripherals
        .iter()
        .filter(|peri| {
            peri["name"]
                .as_str()
                .map(|name| name.contains("CAN"))
                .is_some_and(|name_contains_can| name_contains_can)
        })
        .collect::<Vec<_>>();

    let response = dialoguer::Select::new()
        .with_prompt("Select CAN MASTER peripheral (CAN1 usualy)")
        .items(can_peris.iter().filter_map(|peri| peri["name"].as_str()))
        .interact()?;
    let can_peri = can_peris[response].clone();
    let can_peri_name = can_peri["name"]
        .as_str()
        .ok_or(anyhow::anyhow!("Can peri has no name"))?;

    extra_args.push_str(&format!("\ncan = {}\n", can_peri["name"]));

    // Select TX Pin
    let tx_pins = can_peri["pins"]
        .as_array()
        .ok_or(anyhow::anyhow!("Peripheral has no pins"))?
        .iter()
        .filter(|pin| pin["signal"].as_str() == Some("TX"))
        .collect::<Vec<_>>();

    let response = dialoguer::Select::new()
        .with_prompt("Select CAN TX pin")
        .items(tx_pins.iter().filter_map(|peri| peri["pin"].as_str()))
        .interact()?;
    let tx_pin = tx_pins[response].clone();

    extra_args.push_str(&format!("can_tx = {}\n", tx_pin["pin"]));

    // Select RX Pin
    let tx_pins = can_peri["pins"]
        .as_array()
        .ok_or(anyhow::anyhow!("Peripheral has no pins"))?
        .iter()
        .filter(|pin| pin["signal"].as_str() == Some("RX"))
        .collect::<Vec<_>>();

    let response = dialoguer::Select::new()
        .with_prompt("Select CAN RX pin")
        .items(tx_pins.iter().filter_map(|peri| peri["pin"].as_str()))
        .interact()?;
    let rx_pin = tx_pins[response].clone();

    extra_args.push_str(&format!("can_rx = {}\n", rx_pin["pin"]));

    // Select baudrate
    let baudrate = dialoguer::Input::<u64>::new()
        .with_prompt("Can baudrate")
        .interact_text()?;

    extra_args.push_str(&format!("can_baudrate = \"{}\"\n", baudrate));

    // Interrupts
    let rx0_int = can_peri["interrupts"]
        .as_array()
        .ok_or(anyhow::anyhow!("Can peri has no interrupts"))?
        .iter()
        .filter(|int| int["signal"].as_str() == Some("RX0"))
        .map(|int| int["interrupt"].as_str())
        .next()
        .flatten()
        .ok_or(anyhow::anyhow!("No rx0 interrupt found for can"))?;

    if rx0_int != &format!("{}_RX0", can_peri_name) {
        extra_args.push_str(&format!("\ncan_rx0_int_name = \"{}\"\n", rx0_int));
    }

    let rx1_int = can_peri["interrupts"]
        .as_array()
        .ok_or(anyhow::anyhow!("Can peri has no interrupts"))?
        .iter()
        .filter(|int| int["signal"].as_str() == Some("RX1"))
        .map(|int| int["interrupt"].as_str())
        .next()
        .flatten()
        .ok_or(anyhow::anyhow!("No rx0 interrupt found for can"))?;

    if rx1_int != &format!("{}_RX1", can_peri_name) {
        extra_args.push_str(&format!("can_rx1_int_name = \"{}\"\n", rx1_int));
    }

    let tx_int = can_peri["interrupts"]
        .as_array()
        .ok_or(anyhow::anyhow!("Can peri has no interrupts"))?
        .iter()
        .filter(|int| int["signal"].as_str() == Some("TX"))
        .map(|int| int["interrupt"].as_str())
        .next()
        .flatten()
        .ok_or(anyhow::anyhow!("No rx0 interrupt found for can"))?;

    if tx_int != &format!("{}_TX", can_peri_name) {
        extra_args.push_str(&format!("can_tx_int_name = \"{}\"\n", tx_int));
    }

    let sce_int = can_peri["interrupts"]
        .as_array()
        .ok_or(anyhow::anyhow!("Can peri has no interrupts"))?
        .iter()
        .filter(|int| int["signal"].as_str() == Some("SCE"))
        .map(|int| int["interrupt"].as_str())
        .next()
        .flatten()
        .ok_or(anyhow::anyhow!("No rx0 interrupt found for can"))?;

    if sce_int != &format!("{}_SCE", can_peri_name) {
        extra_args.push_str(&format!("can_sce_int_name = \"{}\"\n", sce_int));
    }

    Ok(())
}

fn interrogate_can_slave_flashing(
    extra_args: &mut String,
    sizes: &ChipSizes,
) -> anyhow::Result<()> {
    // Select CAN Peri
    let can_peris = sizes
        .peripherals
        .iter()
        .filter(|peri| {
            peri["name"]
                .as_str()
                .map(|name| name.contains("CAN"))
                .is_some_and(|name_contains_can| name_contains_can)
        })
        .collect::<Vec<_>>();

    let response = dialoguer::Select::new()
        .with_prompt("Select CAN SLAVE peripheral (CAN2 usualy)")
        .items(can_peris.iter().filter_map(|peri| peri["name"].as_str()))
        .interact()?;
    let can_peri = can_peris[response].clone();

    extra_args.push_str(&format!("\ncan2 = {}\n", can_peri["name"]));

    // Select TX Pin
    let tx_pins = can_peri["pins"]
        .as_array()
        .ok_or(anyhow::anyhow!("Peripheral has no pins"))?
        .iter()
        .filter(|pin| pin["signal"].as_str() == Some("TX"))
        .collect::<Vec<_>>();

    let response = dialoguer::Select::new()
        .with_prompt("Select CAN TX pin")
        .items(tx_pins.iter().filter_map(|peri| peri["pin"].as_str()))
        .interact()?;
    let tx_pin = tx_pins[response].clone();

    extra_args.push_str(&format!("can2_tx = {}\n", tx_pin["pin"]));

    // Select RX Pin
    let tx_pins = can_peri["pins"]
        .as_array()
        .ok_or(anyhow::anyhow!("Peripheral has no pins"))?
        .iter()
        .filter(|pin| pin["signal"].as_str() == Some("RX"))
        .collect::<Vec<_>>();

    let response = dialoguer::Select::new()
        .with_prompt("Select CAN RX pin")
        .items(tx_pins.iter().filter_map(|peri| peri["pin"].as_str()))
        .interact()?;
    let rx_pin = tx_pins[response].clone();

    extra_args.push_str(&format!("can2_rx = {}\n", rx_pin["pin"]));

    interrogate_can_master_flashing(extra_args, sizes)
}
