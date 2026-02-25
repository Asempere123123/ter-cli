mod bootloader;
mod defmt;
mod descriptor;
mod flash;

use std::{
    path::PathBuf,
    process::{Command, Stdio},
    sync::LazyLock,
};

use anyhow::bail;
use clap::{Parser, Subcommand};
use directories::ProjectDirs;

use crate::{bootloader::get_bootloader_path, descriptor::Descriptor};

static DIRS: LazyLock<ProjectDirs> =
    LazyLock::new(|| ProjectDirs::from("com", "ter", "ter-cli").unwrap());

#[derive(Parser)]
#[command(about = "Ter bootloader and flashing CLI for setting up and flashing new projects", long_about = None)]
#[command(version)]
#[command(arg_required_else_help = true)]
struct Cli {
    #[arg(short, long, value_name = "FILE", default_value = "ter.toml")]
    path: PathBuf,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build and Flash binary
    Run {
        #[arg(long)]
        /// Path to the .bin
        bin_path: Option<PathBuf>,
        #[arg(long)]
        /// Attach defmt rtt given an elf file
        defmt: Option<PathBuf>,
        #[arg(long)]
        /// Attach defmt rtt to the bootloader's messages
        bootloader_defmt: bool,
        #[arg(long)]
        /// Bootloader path. For testing purposes
        bootloader_path: Option<PathBuf>,
    },
    /// Flash binary
    Flash {
        #[arg(long)]
        /// Path to the .bin
        bin_path: Option<PathBuf>,
        #[arg(long)]
        /// Attach defmt rtt given an elf file
        defmt: Option<PathBuf>,
        #[arg(long)]
        /// Attach defmt rtt to the bootloader's messages
        bootloader_defmt: bool,
        #[arg(long)]
        /// Bootloader path. For testing purposes
        bootloader_path: Option<PathBuf>,
    },
    /// Clear all bootloader cache
    Clear,
}

fn main() -> anyhow::Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let cli = Cli::parse();
    let descriptor = Descriptor::from_path(cli.path).unwrap();

    match cli.command {
        Commands::Run {
            bin_path,
            defmt,
            bootloader_defmt,
            bootloader_path,
        } => {
            if let Some(build_cmd) = descriptor.build_command() {
                let status = Command::new("sh")
                    .arg("-c")
                    .arg(build_cmd)
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .status()?;

                if !status.success() {
                    bail!("Build command did not run successfully");
                }
            } else {
                bail!("No build_command was specified in ter.toml");
            };

            flash_command(
                bin_path,
                defmt,
                bootloader_defmt,
                bootloader_path,
                &descriptor,
            )?;
        }
        Commands::Flash {
            bin_path,
            defmt,
            bootloader_defmt,
            bootloader_path,
        } => flash_command(
            bin_path,
            defmt,
            bootloader_defmt,
            bootloader_path,
            &descriptor,
        )?,
        Commands::Clear => {
            log::info!("Cleaning up bootloader cache");
            std::fs::remove_dir_all(DIRS.data_dir())?
        }
    }

    Ok(())
}

fn flash_command(
    bin_path: Option<PathBuf>,
    defmt: Option<PathBuf>,
    bootloader_defmt: bool,
    bootloader_path: Option<PathBuf>,
    descriptor: &Descriptor,
) -> anyhow::Result<()> {
    let (bootloader_bin_path, bootloader_elf_path) =
        get_bootloader_path(bootloader_path, &descriptor, bootloader_defmt)?;

    if let Some(bin_path) = bin_path {
        flash::flash(bootloader_bin_path, bin_path, descriptor.chip_name())?;
    } else if let Some(bin_path) = descriptor.bin_path() {
        flash::flash(bootloader_bin_path, bin_path, descriptor.chip_name())?;
    } else {
        log::warn!(
            "No bin path was supplied. You must either pass it with the --bin-path arg or as bin_path in ter.toml"
        )
    }

    if bootloader_defmt {
        defmt::attach_defmt(&descriptor, bootloader_elf_path)?;
    } else if let Some(elf_path) = defmt {
        defmt::attach_defmt(&descriptor, elf_path)?;
    } else if let Some(elf_path) = descriptor.elf_path() {
        defmt::attach_defmt(&descriptor, elf_path.to_path_buf())?;
    }

    Ok(())
}
