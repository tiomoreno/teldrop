use anyhow::{bail, Context, Result};
use grammers_client::{session::Session, Client, Config};

use crate::config::Config as AppConfig;

pub async fn connect(app_config: &AppConfig) -> Result<Client> {
    let session_path = AppConfig::session_path()?;

    let session =
        Session::load_file_or_create(&session_path).context("Failed to load session file")?;

    let client = Client::connect(Config {
        session,
        api_id: app_config.api_id,
        api_hash: app_config.api_hash.clone(),
        params: Default::default(),
    })
    .await
    .context("Failed to connect to Telegram servers")?;

    Ok(client)
}

pub fn save_session(client: &Client) -> Result<()> {
    let session_path = AppConfig::session_path()?;
    client
        .session()
        .save_to_file(&session_path)
        .context("Failed to save session")?;
    Ok(())
}

pub async fn require_auth(client: &Client) -> Result<()> {
    if !client.is_authorized().await? {
        bail!("Not logged in. Run `teldrop login` first.");
    }
    Ok(())
}
