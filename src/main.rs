use teloxide::prelude::*;
use teloxide::sugar::request::RequestReplyExt;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting echo bot with reply functionality...");

    let bot = Bot::from_env();

    teloxide::repl(bot, |bot: Bot, msg: Message| async move {
        // Extract the text from the message  
        if let Some(text) = msg.text() {
            // Reply to the message with the same text  
            bot.send_message(msg.chat.id, text)
                .reply_to(msg)
                .await?;
        }
        Ok(())
    })
        .await;
}