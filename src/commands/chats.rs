use anyhow::Result;
use grammers_client::types::Chat;

use crate::config::Config;
use crate::telegram;

pub async fn run(filter: Option<String>) -> Result<()> {
    let config = Config::load_or_prompt()?;
    let client = telegram::connect(&config).await?;
    telegram::require_auth(&client).await?;

    println!("{:<20} {:<12} {}", "TYPE", "ID", "NAME");
    println!("{}", "-".repeat(70));

    let filter = filter.map(|f| f.to_lowercase());
    let mut count = 0;

    let mut dialogs = client.iter_dialogs();

    while let Some(dialog) = dialogs.next().await? {
        let chat = dialog.chat();
        let name = chat.name();

        if let Some(ref f) = filter {
            if !name.to_lowercase().contains(f.as_str()) {
                continue;
            }
        }

        let kind = chat_kind(&chat);
        let id = chat.id();

        println!("{:<20} {:<12} {}", kind, id, name);
        count += 1;
    }

    println!();
    println!("{} chat(s) found.", count);

    Ok(())
}

fn chat_kind(chat: &Chat) -> &'static str {
    match chat {
        Chat::User(_) => "Private",
        Chat::Group(_) => "Group",
        Chat::Channel(_) => "Channel",
    }
}
