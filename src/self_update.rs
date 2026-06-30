use self_update::cargo_crate_version;

pub fn update_self() -> anyhow::Result<()> {
    log::warn!("Self-Updating might require sudo privileges");

    let status = self_update::backends::github::Update::configure()
        .repo_owner("Asempere123123")
        .repo_name("ter-cli")
        .bin_name("ter")
        .target("")
        .show_download_progress(true)
        .current_version(cargo_crate_version!())
        .build()?
        .update()?;

    log::info!("Self-Update succesfull: {}", status);

    Ok(())
}
