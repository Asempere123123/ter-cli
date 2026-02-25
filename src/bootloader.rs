use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

use crate::{DIRS, descriptor::Descriptor};
// CHANGE
pub fn get_bootloader_path(
    descriptor: &Descriptor,
    defmt: bool,
) -> anyhow::Result<(PathBuf, PathBuf)> {
    // END CHANGE
    let _ = std::fs::create_dir_all(DIRS.data_dir());

    let mut dir_name = descriptor.chip_hal_name();
    if defmt {
        dir_name.push_str("-defmt");
    }
    let target_dir = DIRS.data_dir().join(&dir_name);

    if !target_dir.is_dir() {
        log::info!("Bootloader for current chip not found. Building new bootloader");
        if let Err(e) = generate_new_bootloader(descriptor, defmt) {
            let _ = std::fs::remove_dir_all(&target_dir);
            return Err(e);
        }
        log::info!("Bootloader built");
    }

    // CHANGE
    if !target_dir.join("boot.bin").exists() || !target_dir.join("boot.elf").exists() {
        let _ = std::fs::remove_dir_all(&target_dir);
        log::info!("Bootloader for current chip was corrupt. Building new bootloader");
        if let Err(e) = generate_new_bootloader(descriptor, defmt) {
            let _ = std::fs::remove_dir_all(&target_dir);
            return Err(e);
        }
        log::info!("Bootloader built");
    }

    Ok((target_dir.join("boot.bin"), target_dir.join("boot.elf")))
}
// END CHANGE

fn generate_new_bootloader(descriptor: &Descriptor, defmt: bool) -> anyhow::Result<()> {
    let mut dir_name = descriptor.chip_hal_name();
    if defmt {
        dir_name.push_str("-defmt");
    }

    let status = Command::new("cargo")
        .args([
            "generate",
            "gh:Asempere123123/stm32-bootloader",
            "--name",
            &dir_name,
            "-d",
            &format!("chip-name={}", descriptor.chip_name()),
            "-d",
            &format!("chip-hal-name={}", descriptor.chip_hal_name()),
            "-d",
            &format!("chip-arch={}", descriptor.chip_arch_name()?),
        ])
        .current_dir(DIRS.data_dir())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    if !status.success() {
        anyhow::bail!("Cargo generate failed with status: {}", status);
    }

    let bootloader_dir = DIRS.data_dir().join(&dir_name);

    let mut objcopy_args = vec!["objcopy", "--release"];
    if defmt {
        objcopy_args.push("-F");
        objcopy_args.push("defmt");
    }
    objcopy_args.extend(["--", "-O", "binary", "boot.bin"]);
    let status = Command::new("cargo")
        .args(&objcopy_args)
        .current_dir(&bootloader_dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    // CHANGE
    if !status.success() {
        anyhow::bail!("Cargo objcopy failed with status: {}", status);
    }

    let chip_arch = descriptor.chip_arch_name()?;
    let elf_src = bootloader_dir
        .join("target")
        .join(&chip_arch)
        .join("release")
        .join("boot");

    let elf_dest = bootloader_dir.join("boot.elf");

    if let Err(e) = std::fs::copy(&elf_src, &elf_dest) {
        anyhow::bail!(
            "Failed to copy ELF file from {} to {}: {}",
            elf_src.display(),
            elf_dest.display(),
            e
        );
    }

    let status = Command::new("cargo")
        // END CHANGE
        .arg("clean")
        .current_dir(&bootloader_dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    if !status.success() {
        anyhow::bail!("Cargo clean failed with status: {}", status);
    }

    Ok(())
}
