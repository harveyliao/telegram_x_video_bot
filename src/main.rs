mod bot;
mod config;
mod storage;
mod ytdlp;

use anyhow::Result;
use config::AppConfig;

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();
    log::info!("Starting X video downloader bot...");

    let cfg = AppConfig::from_env()?;
    bot::run(cfg).await?;
    Ok(())
}

