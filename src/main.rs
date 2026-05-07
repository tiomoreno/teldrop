use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod commands;
mod config;
mod media;
mod telegram;
mod tui;

#[derive(Parser)]
#[command(
    name = "rustgram",
    version,
    about = "Telegram channel/chat downloader\n\nRun without arguments or use `tui` for the interactive interface.\nAll other subcommands work as a conventional CLI (suitable for scripts and skills)."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Launch the interactive TUI
    Tui,

    /// Authenticate with Telegram (phone + OTP + optional 2FA)
    Login,

    /// Remove saved session (logout)
    Logout,

    /// List your chats, groups, and channels
    Chats {
        /// Filter chats by name (case-insensitive substring)
        #[arg(short, long)]
        filter: Option<String>,
    },

    /// Download media from a chat or channel
    Download {
        /// Chat username (e.g. channelname or @channelname) or numeric chat ID
        chat: String,

        /// Output directory [default: ~/Downloads/rustgram/<chat_name>]
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Filter by media type: all, photo, video, document, audio
        #[arg(short = 't', long, default_value = "all")]
        media_type: String,

        /// Maximum number of messages to scan
        #[arg(short, long)]
        limit: Option<usize>,

        /// Search query to filter messages
        #[arg(short, long)]
        query: Option<String>,

        /// Skip files that already exist in the output directory
        #[arg(long, default_value_t = true)]
        skip_existing: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Tui => {
            let config = config::Config::load_or_prompt()?;
            tui::run(config).await
        }
        Commands::Login => commands::login::run().await,
        Commands::Logout => commands::logout::run().await,
        Commands::Chats { filter } => commands::chats::run(filter).await,
        Commands::Download {
            chat,
            output,
            media_type,
            limit,
            query,
            skip_existing,
        } => commands::download::run(chat, output, media_type, limit, query, skip_existing).await,
    }
}
