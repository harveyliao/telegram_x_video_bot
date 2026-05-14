use anyhow::Result;
use teloxide::prelude::*;
use teloxide::sugar::request::RequestReplyExt;
use teloxide::types::{InputFile, UserId};

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
            let sender_user_id = msg.from.as_ref().map(|user| user.id);
            if !is_user_allowed(sender_user_id, &cfg.allowed_user_ids) {
                log::debug!(
                    "Ignoring message from unauthorized sender: chat_id={}, sender_user_id={:?}",
                    msg.chat.id.0,
                    sender_user_id.map(|id| id.0)
                );
                return Ok(());
            }

            if let Some(text) = msg.text() {
                if text.contains("twitter.com") || text.contains("x.com") {
                    let processing_msg = bot
                        .send_message(msg.chat.id, "⏳ Downloading video...")
                        .reply_to(&msg)
                        .await?;

                    // 每个任务独立目录
                    let (task_dir, _) = match storage::make_task_dir(&cfg.video_dir, msg.chat.id.0)
                    {
                        Ok(v) => v,
                        Err(e) => {
                            bot.send_message(
                                msg.chat.id,
                                format!("❌ Failed to create output folder: {e}"),
                            )
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

fn is_user_allowed(
    sender_user_id: Option<UserId>,
    allowed_user_ids: &std::collections::HashSet<UserId>,
) -> bool {
    sender_user_id
        .map(|id| allowed_user_ids.contains(&id))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn allows_whitelisted_user_id() {
        let mut allowed = HashSet::new();
        allowed.insert(UserId(42));
        assert!(is_user_allowed(Some(UserId(42)), &allowed));
    }

    #[test]
    fn denies_non_whitelisted_user_id() {
        let mut allowed = HashSet::new();
        allowed.insert(UserId(42));
        assert!(!is_user_allowed(Some(UserId(7)), &allowed));
    }

    #[test]
    fn denies_when_sender_is_missing() {
        let mut allowed = HashSet::new();
        allowed.insert(UserId(42));
        assert!(!is_user_allowed(None, &allowed));
    }
}
