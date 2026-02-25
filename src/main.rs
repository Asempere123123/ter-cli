mod bootloader;
mod defmt;
mod descriptor;
mod flash;

use std::{path::PathBuf, sync::LazyLock};

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
        Commands::Flash {
            bin_path,
            defmt,
            bootloader_defmt,
            bootloader_path,
        } => {
            let (bootloader_bin_path, bootloader_elf_path) =
                get_bootloader_path(bootloader_path, &descriptor, bootloader_defmt)?;
            flash::flash(
                bootloader_bin_path,
                bin_path.unwrap(),
                descriptor.chip_name(),
            )?;

            if bootloader_defmt {
                println!("{:?}", &bootloader_elf_path);
                defmt::attach_defmt(&descriptor, bootloader_elf_path)?;
            } else if let Some(elf_path) = defmt {
                defmt::attach_defmt(&descriptor, elf_path)?;
            }
        }
        Commands::Clear => {
            log::info!("Cleaning up bootloader cache");
            std::fs::remove_dir_all(DIRS.data_dir())?
        }
    }

    Ok(())
}
