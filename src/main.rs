use chrono::Local;
use std::fs;
use std::path::Path;
use std::process::Command;
use teloxide::prelude::*;
use teloxide::sugar::request::RequestReplyExt;
use teloxide::types::InputFile;

#[tokio::main]
async fn main() {
    // Initialize logging
    pretty_env_logger::init();
    log::info!("Starting Twitter video downloader bot...");

    // Check TELOXIDE_TOKEN
    let token = match std::env::var("TELOXIDE_TOKEN") {
        Ok(token) => {
            log::info!("TELOXIDE_TOKEN found (length: {})", token.len());
            token
        }
        Err(e) => {
            log::error!("Failed to read TELOXIDE_TOKEN: {}", e);
            std::process::exit(1);
        }
    };

    // Initialize bot
    log::info!("Creating bot...");
    let bot = Bot::new(&token);

    // Test Telegram API connection
    match bot.get_me().await {
        Ok(me) => log::info!("Connected to Telegram API. Bot info: {:?}", me),
        Err(e) => {
            log::error!("Failed to connect to Telegram API: {}", e);
            std::process::exit(1);
        }
    }

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
                        "--cookies",
                        "twitter.txt",
                        "-o",
                        &filename,
                        text, // user-provided URL
                    ])
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);

                if success && path.exists() {
                    // Read the file
                    let video_file = InputFile::file(path.to_path_buf());

                    // Send it back
                    bot.send_video(msg.chat.id, video_file)
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
