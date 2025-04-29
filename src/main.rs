use chrono::Local;
use std::fs;
use std::path::Path;
use std::process::Command;
use teloxide::prelude::*;
use teloxide::sugar::request::RequestReplyExt;
use teloxide::types::InputFile;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting Twitter video downloader bot...");

    let bot = Bot::from_env();

    teloxide::repl(bot, |bot: Bot, msg: Message| async move {
        if let Some(text) = msg.text() {
            if text.contains("twitter.com") || text.contains("x.com") {
                let processing_msg = bot.send_message(msg.chat.id, "⏳ Downloading video...")
                    .reply_to(&msg)
                    .await?;

                let now = Local::now();
                let formatted_time = now.format("%Y-%m-%d_%H-%M-%S");
                let out_dir = "video";
                let filename = format!("{}/video_{}.mp4", out_dir, formatted_time);
                let path = Path::new(&filename);

                // Create directory if not exists
                fs::create_dir_all(out_dir).expect("Failed to create video directory");

                // Run yt-dlp command
                let success = Command::new("yt-dlp")
                    .args([
                        "-f",
                        "best[ext=mp4]",
                        "-o",
                        &filename,
                        text, // user-provided URL
                    ])
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);

                if success && path.exists() {
                    // Read the file
                    // let video_data = fs::read(path).expect("Failed to read downloaded video");
                    let video_file = InputFile::file(path.to_path_buf());

                    // Send it back
                    bot.send_video(msg.chat.id, video_file)
                        .caption("📹 Here's your video!")
                        .reply_to(msg.id)
                        .await?;

                    // delete the processing message after sending back
                    bot.delete_message(msg.chat.id, processing_msg.id)
                        .await?;
                } else {
                    bot.send_message(msg.chat.id, "❌ Failed to download or locate the video.")
                        .reply_to(&msg)
                        .await?;
                }
            } else {
                bot.send_message(msg.chat.id, "Please provide a valid Twitter/X video link.")
                    .reply_to(&msg)
                    .await?;
            }
        }

        Ok(())
    })
        .await;
}
