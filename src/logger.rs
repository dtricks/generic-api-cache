use color_eyre::eyre::Result;

pub fn init() -> Result<()> {
    log4rs::init_file("log4rs.yml", Default::default())
        .map_err(|e| color_eyre::eyre::eyre!("{:?}", e))?;

    log::info!("Initialized Logger");

    Ok(())
}
