use anyhow::Result;
use std::fs;

use crate::config::Config;

pub async fn run() -> Result<()> {
    let session_path = Config::session_path()?;

    if !session_path.exists() {
        println!("No active session found.");
        return Ok(());
    }

    fs::remove_file(&session_path)?;
    println!("Session removed. You are now logged out.");

    Ok(())
}
