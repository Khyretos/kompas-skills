//! Sends plain-text mail through the admin's SMTP server. The password comes
//! from the environment (SMTP_PASSWORD) and is never stored or logged.

use anyhow::{Context, Result};
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Mailbox, header::ContentType},
    transport::smtp::authentication::Credentials,
};

pub struct SmtpSettings {
    pub host: String,
    pub port: u16,
    /// "starttls", "tls" (implicit TLS) or "none".
    pub tls: String,
    pub user: String,
    pub from: String,
}

pub async fn send(s: &SmtpSettings, password: Option<&str>, to: &str, subject: &str, body: &str) -> Result<()> {
    let builder = match s.tls.as_str() {
        "tls" => AsyncSmtpTransport::<Tokio1Executor>::relay(&s.host),
        "starttls" => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&s.host),
        _ => Ok(AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&s.host)),
    }
    .with_context(|| format!("mail server {}", s.host))?
    .port(s.port);
    let builder = match password {
        Some(p) if !s.user.is_empty() => builder.credentials(Credentials::new(s.user.clone(), p.to_string())),
        _ => builder,
    };
    let message = Message::builder()
        .from(s.from.parse::<Mailbox>().context("sender address")?)
        .to(to.parse::<Mailbox>().context("recipient address")?)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(body.to_string())
        .context("building the mail")?;
    builder.build().send(message).await.context("sending")?;
    Ok(())
}
