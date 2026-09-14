//! SMTP mailer.
//!
//! Tiny wrapper around `lettre` that sends plain-text emails over
//! STARTTLS. The transport is built once at startup and reused.

use lettre::{
    message::header::ContentType,
    transport::smtp::{authentication::Credentials, AsyncSmtpTransport},
    AsyncTransport, Message, Tokio1Executor,
};

use crate::{
    config::Config,
    error::{WebError, WebResult},
};

/// Pre-built async SMTP mailer.
#[derive(Clone)]
pub struct Mailer {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: String,
}

impl Mailer {
    /// Build a mailer from the application config.
    ///
    /// # Errors
    /// Returns `Internal` if the SMTP relay cannot be initialized
    /// (typically a malformed host or TLS setup failure).
    pub fn new(cfg: &Config) -> WebResult<Self> {
    let creds = Credentials::new(cfg.smtp_username.clone(), cfg.smtp_password.clone());
    let transport = if cfg.is_production {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.smtp_host)
            .map_err(|e| WebError::Internal(anyhow::anyhow!("smtp init: {e}")))?
            .port(cfg.smtp_port)
            .credentials(creds)
            .build()
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&cfg.smtp_host)
            .port(cfg.smtp_port)
            .build()
    };
    Ok(Self {              // ← this line was missing
        transport,
        from: cfg.smtp_from.clone(),
    })
}
    /// Send the OTP code to the given Epitech address.
    ///
    /// # Errors
    /// Returns `Upstream` on any SMTP failure.
    pub async fn send_otp(&self, to: &str, code: &str) -> WebResult<()> {
        let body = format!(
            "Bonjour,\n\n\
             Votre code de vérification GameCloud OS est : {code}\n\n\
             Ce code expire dans 15 minutes.\n\n\
             Si vous n'êtes pas à l'origine de cette demande, ignorez cet e-mail.\n\n\
             — L'équipe GameCloud"
        );
        let email = Message::builder()
            .from(
                self.from
                    .parse()
                    .map_err(|e| WebError::Internal(anyhow::anyhow!("from parse: {e}")))?,
            )
            .to(to
                .parse()
                .map_err(|e| WebError::Internal(anyhow::anyhow!("to parse: {e}")))?)
            .subject("[GameCloud OS] Code de vérification")
            .header(ContentType::TEXT_PLAIN)
            .body(body)
            .map_err(|e| WebError::Internal(anyhow::anyhow!("email build: {e}")))?;

        self.transport
            .send(email)
            .await
            .map_err(|e| WebError::Upstream(format!("smtp: {e}")))?;
        Ok(())
    }
}
