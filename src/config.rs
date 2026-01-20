use anyhow::{bail, Result};
use secrecy::{ExposeSecret, SecretString};

#[derive(Clone)]
pub struct AppConfig {
    pub teloxide_token: SecretString,
    pub cookie_file: String,
    pub video_dir: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let token = std::env::var("TELOXIDE_TOKEN")
            .map(|s: String| SecretString::new(s.into_boxed_str()))
            .map_err(|_| anyhow::anyhow!("Missing env TELOXIDE_TOKEN"))?;

        let cookie_file = std::env::var("COOKIE_FILE")
            .map_err(|_| anyhow::anyhow!("Missing env COOKIE_FILE"))?;

        let video_dir = std::env::var("VIDEO_DIR").unwrap_or_else(|_| "video".to_string());

        if !std::path::Path::new(&cookie_file).exists() {
            bail!("COOKIE_FILE does not exist: {}", cookie_file);
        }

        Ok(Self {
            teloxide_token: token,
            cookie_file,
            video_dir,
        })
    }

    pub fn token_str(&self) -> &str {
        self.teloxide_token.expose_secret()
    }
}

