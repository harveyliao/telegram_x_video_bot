use std::collections::HashSet;

use anyhow::{bail, Context, Result};
use secrecy::{ExposeSecret, SecretString};
use teloxide::types::UserId;

#[derive(Clone)]
pub struct AppConfig {
    pub teloxide_token: SecretString,
    pub cookie_file: String,
    pub video_dir: String,
    pub allowed_user_ids: HashSet<UserId>,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let token = std::env::var("TELOXIDE_TOKEN")
            .map(|s: String| SecretString::new(s.into_boxed_str()))
            .map_err(|_| anyhow::anyhow!("Missing env TELOXIDE_TOKEN"))?;

        let cookie_file =
            std::env::var("COOKIE_FILE").map_err(|_| anyhow::anyhow!("Missing env COOKIE_FILE"))?;

        let video_dir = std::env::var("VIDEO_DIR").unwrap_or_else(|_| "video".to_string());
        let allowed_user_ids = parse_allowed_user_ids_from_env()?;

        if !std::path::Path::new(&cookie_file).exists() {
            bail!("COOKIE_FILE does not exist: {}", cookie_file);
        }

        Ok(Self {
            teloxide_token: token,
            cookie_file,
            video_dir,
            allowed_user_ids,
        })
    }

    pub fn token_str(&self) -> &str {
        self.teloxide_token.expose_secret()
    }
}

fn parse_allowed_user_ids_from_env() -> Result<HashSet<UserId>> {
    let raw = std::env::var("ALLOWED_USER_IDS")
        .map_err(|_| anyhow::anyhow!("Missing env ALLOWED_USER_IDS"))?;
    parse_allowed_user_ids(&raw)
}

fn parse_allowed_user_ids(raw: &str) -> Result<HashSet<UserId>> {
    let mut out = HashSet::new();

    for token in raw.split(',') {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            bail!("ALLOWED_USER_IDS contains an empty entry");
        }

        let id = trimmed
            .parse::<u64>()
            .with_context(|| format!("ALLOWED_USER_IDS has invalid user id: {trimmed}"))?;
        out.insert(UserId(id));
    }

    if out.is_empty() {
        bail!("ALLOWED_USER_IDS must contain at least one user id");
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn parse_allowed_user_ids_valid_with_spaces() {
        let got = parse_allowed_user_ids("123, 456 ,789").expect("should parse");
        assert!(got.contains(&UserId(123)));
        assert!(got.contains(&UserId(456)));
        assert!(got.contains(&UserId(789)));
        assert_eq!(got.len(), 3);
    }

    #[test]
    fn parse_allowed_user_ids_fails_on_empty_entries() {
        let err = parse_allowed_user_ids("123,,456")
            .expect_err("empty entry should fail")
            .to_string();
        assert!(err.contains("empty entry"));
    }

    #[test]
    fn parse_allowed_user_ids_fails_on_non_numeric_token() {
        let err = parse_allowed_user_ids("123,abc")
            .expect_err("invalid token should fail")
            .to_string();
        assert!(err.contains("invalid user id"));
        assert!(err.contains("abc"));
    }

    #[test]
    fn from_env_fails_when_allowed_user_ids_missing() {
        let _guard = ENV_LOCK.lock().expect("lock poisoned");
        let cookie_path = make_temp_cookie_file();

        let prior_token = std::env::var("TELOXIDE_TOKEN").ok();
        let prior_cookie = std::env::var("COOKIE_FILE").ok();
        let prior_video_dir = std::env::var("VIDEO_DIR").ok();
        let prior_allowed = std::env::var("ALLOWED_USER_IDS").ok();

        std::env::set_var("TELOXIDE_TOKEN", "token");
        std::env::set_var("COOKIE_FILE", cookie_path.to_string_lossy().as_ref());
        std::env::set_var("VIDEO_DIR", "video");
        std::env::remove_var("ALLOWED_USER_IDS");

        let result = AppConfig::from_env();
        assert!(result.is_err(), "missing ALLOWED_USER_IDS must fail");
        let err = result
            .err()
            .expect("error should be present")
            .to_string();
        assert!(err.contains("Missing env ALLOWED_USER_IDS"));

        restore_env_var("TELOXIDE_TOKEN", prior_token.as_deref());
        restore_env_var("COOKIE_FILE", prior_cookie.as_deref());
        restore_env_var("VIDEO_DIR", prior_video_dir.as_deref());
        restore_env_var("ALLOWED_USER_IDS", prior_allowed.as_deref());

        let _ = std::fs::remove_file(cookie_path);
    }

    fn restore_env_var(name: &str, value: Option<&str>) {
        if let Some(v) = value {
            std::env::set_var(name, v);
        } else {
            std::env::remove_var(name);
        }
    }

    fn make_temp_cookie_file() -> PathBuf {
        let mut path = std::env::temp_dir();
        let unique = format!(
            "xbot-config-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time went backwards")
                .as_nanos()
        );
        path.push(unique);
        std::fs::write(&path, b"cookie").expect("failed to create cookie file");
        path
    }
}
