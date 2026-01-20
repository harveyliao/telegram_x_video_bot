use anyhow::Result;
use chrono::Local;
use std::path::PathBuf;

pub fn make_task_dir(base_dir: &str, chat_id: i64) -> Result<(PathBuf, String)> {
    let ts = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let dirname = format!("{}_{}", ts, chat_id);
    let dir = PathBuf::from(base_dir).join(dirname);
    std::fs::create_dir_all(&dir)?;
    Ok((dir, ts))
}

