use anyhow::{Context, Result};
use tokio::process::Command;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub async fn download(url: &str, cookie_file: &str, out_dir: &Path) -> Result<PathBuf> {
    // 1) 确保 out_dir 存在
    tokio::fs::create_dir_all(out_dir)
        .await
        .with_context(|| format!("Failed to create output dir: {}", out_dir.display()))?;

    // 2) 把只读 secrets cookie 复制到可写目录，避免 yt-dlp save_cookies() 失败
    let cookie_tmp = out_dir.join("cookie.txt");
    tokio::fs::copy(cookie_file, &cookie_tmp)
        .await
        .with_context(|| format!("Failed to copy cookie file from {cookie_file}"))?;

    // 设置 cookie 副本权限（600）
    #[cfg(unix)]
    {
        let perm = std::fs::Permissions::from_mode(0o600);
        tokio::fs::set_permissions(&cookie_tmp, perm)
            .await
            .context("Failed to set permissions on cookie temp file")?;
    }

    // 3) 输出模板：只生成 video.<ext>，便于后续查找
    let out_tpl = out_dir.join("video.%(ext)s");
    let out_tpl_str = out_tpl.to_string_lossy().to_string();

    let cookie_tmp_str = cookie_tmp.to_string_lossy().to_string();

    let output = Command::new("yt-dlp")
        .args([
            "-f",
            "best[ext=mp4]/best",
            "--cookies",
            &cookie_tmp_str,
            "-o",
            &out_tpl_str,
            url,
        ])
        .output()
        .await
        .context("Failed to spawn yt-dlp")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("yt-dlp failed: {}", trim(&stderr, 1200));
    }

    // 4) 找到 video.<ext> 输出文件（避免把 cookie.txt 误认为输出）
    let mut rd = tokio::fs::read_dir(out_dir)
        .await
        .context("Failed to read output dir")?;

    while let Some(ent) = rd.next_entry().await? {
        let p = ent.path();
        if !p.is_file() {
            continue;
        }

        let name = ent.file_name();
        let name = name.to_string_lossy();

        // 只认 video.xxx，并且排除 .part 之类的中间文件
        if name.starts_with("video.")
            && !name.ends_with(".part")
            && name != "cookie.txt"
        {
            return Ok(p);
        }
    }

    anyhow::bail!("yt-dlp succeeded but no output video file found")
}

fn trim(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}


