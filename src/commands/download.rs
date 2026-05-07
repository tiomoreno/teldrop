use anyhow::{bail, Context, Result};
use grammers_client::types::{Chat, Downloadable};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncWriteExt;

use crate::config::Config;
use crate::media;
use crate::telegram;

pub async fn run(
    chat_arg: String,
    output: Option<PathBuf>,
    media_type: String,
    limit: Option<usize>,
    query: Option<String>,
    skip_existing: bool,
) -> Result<()> {
    let config = Config::load_or_prompt()?;
    let client = telegram::connect(&config).await?;
    telegram::require_auth(&client).await?;

    let chat = resolve_chat(&client, &chat_arg).await?;
    let chat_name = chat.name().to_string();
    println!("Chat: {} (ID: {})", chat_name, chat.id());

    let output_dir = match output {
        Some(p) => p,
        None => {
            let downloads = dirs::download_dir().unwrap_or_else(|| PathBuf::from("."));
            downloads
                .join("rustgram")
                .join(media::sanitize_name(&chat_name))
        }
    };
    std::fs::create_dir_all(&output_dir).context("Failed to create output directory")?;
    println!("Output: {}", output_dir.display());
    println!();

    let packed = chat.pack();
    let mut iter = client.iter_messages(packed);
    if let Some(n) = limit {
        iter = iter.limit(n);
    }

    let query_lower = query.as_deref().map(str::to_lowercase);

    let mut downloaded = 0usize;
    let mut skipped = 0usize;
    let mut scanned = 0usize;

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::with_template("{spinner:.green} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    spinner.enable_steady_tick(Duration::from_millis(80));

    while let Some(message) = iter.next().await? {
        scanned += 1;
        spinner.set_message(format!(
            "Scanning… ({} scanned, {} downloaded)",
            scanned, downloaded
        ));

        if let Some(ref q) = query_lower {
            if !message.text().to_lowercase().contains(q.as_str()) {
                continue;
            }
        }

        let Some(media) = message.media() else {
            continue;
        };

        if !media::is_downloadable(&media) {
            continue;
        }
        if !media::matches_filter(&media, &media_type) {
            continue;
        }

        let filename = media::media_filename(&media, &message);
        let output_path = output_dir.join(&filename);

        if skip_existing && output_path.exists() {
            skipped += 1;
            continue;
        }

        let file_size = media::media_size(&media);
        let downloadable = Downloadable::Media(media);

        spinner.suspend(|| {
            println!("  ↓ {}", filename);
        });

        download_with_progress(&client, &downloadable, file_size, &output_path).await?;
        downloaded += 1;
    }

    spinner.finish_and_clear();

    println!();
    println!(
        "Done. {} downloaded, {} skipped, {} messages scanned.",
        downloaded, skipped, scanned
    );

    Ok(())
}

async fn resolve_chat(client: &grammers_client::Client, arg: &str) -> Result<Chat> {
    let clean = arg.trim_start_matches('@');

    if let Ok(id) = clean.parse::<i64>() {
        let mut dialogs = client.iter_dialogs();
        while let Some(dialog) = dialogs.next().await? {
            if dialog.chat().id() == id {
                return Ok(dialog.chat().clone());
            }
        }
        bail!("Chat with ID {} not found in your dialog list.", id);
    }

    match client.resolve_username(clean).await? {
        Some(chat) => Ok(chat),
        None => bail!("No chat found with username '{}'.", clean),
    }
}

async fn download_with_progress(
    client: &grammers_client::Client,
    downloadable: &Downloadable,
    file_size: u64,
    output_path: &Path,
) -> Result<()> {
    let pb = if file_size > 0 {
        let pb = ProgressBar::new(file_size);
        pb.set_style(
            ProgressStyle::with_template("     [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .unwrap()
                .progress_chars("=>-"),
        );
        pb
    } else {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::with_template("     {spinner:.cyan} {bytes} downloaded")
                .unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        pb.enable_steady_tick(Duration::from_millis(80));
        pb
    };

    let mut file = tokio::fs::File::create(output_path)
        .await
        .context("Failed to create output file")?;

    let mut download = client.iter_download(downloadable);

    while let Ok(Some(chunk)) = download.next().await {
        file.write_all(&chunk)
            .await
            .context("Failed to write to file")?;
        pb.inc(chunk.len() as u64);
    }

    file.flush().await.context("Failed to flush file")?;
    pb.finish_and_clear();

    Ok(())
}
