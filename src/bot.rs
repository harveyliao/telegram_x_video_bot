use anyhow::Result;
use teloxide::prelude::*;
use teloxide::types::InputFile;
use teloxide::sugar::request::RequestReplyExt;

use crate::{config::AppConfig, storage, ytdlp};

pub async fn run(cfg: AppConfig) -> Result<()> {
    let bot = Bot::new(cfg.token_str());

    // （可选）测试连通性
    if let Ok(me) = bot.get_me().await {
        log::info!("Connected to Telegram API: @{}", me.username());
    }

    teloxide::repl(bot, move |bot: Bot, msg: Message| {
        let cfg = cfg.clone();
        async move {
            if let Some(text) = msg.text() {
                if text.contains("twitter.com") || text.contains("x.com") {
                    let processing_msg = bot
                        .send_message(msg.chat.id, "⏳ Downloading video...")
                        .reply_to(&msg)
                        .await?;

                    // 每个任务独立目录
                    let (task_dir, _) = match storage::make_task_dir(&cfg.video_dir, msg.chat.id.0) {
                        Ok(v) => v,
                        Err(e) => {
                            bot.send_message(msg.chat.id, format!("❌ Failed to create output folder: {e}"))
                                .reply_to(&msg)
                                .await?;
                            // 也可以尝试删掉 processing message（可选）
                            let _ = bot.delete_message(msg.chat.id, processing_msg.id).await;
                            return Ok(());
                        }
                    };

                    match ytdlp::download(text, &cfg.cookie_file, &task_dir).await {
                        Ok(path) => {
                            let video_file = InputFile::file(path.clone());
                            bot.send_video(msg.chat.id, video_file)
                                .reply_to(msg.id)
                                .await?;

                            bot.delete_message(msg.chat.id, processing_msg.id).await?;
                        }
                        Err(e) => {
                            bot.send_message(msg.chat.id, format!("❌ Failed: {}", e))
                                .reply_to(&msg)
                                .await?;
                        }
                    }
                } else {
                    bot.send_message(msg.chat.id, "Please provide a valid Twitter/X video link.")
                        .reply_to(&msg)
                        .await?;
                }
            }
            Ok(())
        }
    })
    .await;

    Ok(())
}

