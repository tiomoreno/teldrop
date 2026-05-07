use anyhow::{Context, Result};
use dialoguer::{Input, Password};
use grammers_client::SignInError;

use crate::config::Config;
use crate::telegram;

pub async fn run() -> Result<()> {
    let config = Config::load_or_prompt()?;
    let client = telegram::connect(&config).await?;

    if client.is_authorized().await? {
        println!("Already logged in.");
        return Ok(());
    }

    let phone: String = Input::new()
        .with_prompt("Phone number (with country code, e.g. +5511999999999)")
        .interact_text()
        .context("Failed to read phone number")?;

    println!("Sending verification code to {}...", phone);

    let token = client
        .request_login_code(&phone)
        .await
        .context("Failed to request login code. Check your phone number.")?;

    let code: String = Input::new()
        .with_prompt("Verification code")
        .interact_text()
        .context("Failed to read verification code")?;

    match client.sign_in(&token, &code).await {
        Ok(user) => {
            let name = user.full_name();
            let username = user.username().unwrap_or("no username");
            println!("Logged in as: {} (@{})", name, username);
        }
        Err(SignInError::PasswordRequired(password_token)) => {
            let hint = password_token.hint().unwrap_or("none");
            let password = Password::new()
                .with_prompt(format!("2FA password (hint: {})", hint))
                .interact()
                .context("Failed to read 2FA password")?;

            client
                .check_password(password_token, password.trim())
                .await
                .context("2FA authentication failed. Wrong password?")?;

            println!("Logged in with 2FA.");
        }
        Err(e) => return Err(anyhow::anyhow!("Sign in failed: {}", e)),
    }

    telegram::save_session(&client)?;
    println!("Session saved. You can now use other commands.");

    Ok(())
}
