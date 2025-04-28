use anyhow::Result;
use std::process::Command;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    // Create output directory if it doesn't exist
    let output_dir = PathBuf::from("output");
    if !output_dir.exists() {
        std::fs::create_dir(&output_dir)?;
    }

    // Example URL - replace with your desired video URL
    let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
    
    // Build the yt-dlp command
    let output = Command::new("yt-dlp")
        .arg(url)
        .arg("-o")
        .arg(output_dir.join("%(title)s.%(ext)s"))
        .output()?;

    if output.status.success() {
        println!("Video downloaded successfully!");
        println!("Output: {}", String::from_utf8_lossy(&output.stdout));
    } else {
        eprintln!("Error downloading video:");
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
    }

    Ok(())
}
