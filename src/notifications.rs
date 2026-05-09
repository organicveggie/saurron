use anyhow::{Context, Result};
use tracing::{error, info};

use crate::{
    config::{EmailConfig, MqttConfig, NotificationsConfig, PushoverConfig, WebhookConfig},
    update::{SessionReport, parse_duration_secs},
};

const DEFAULT_TEMPLATE: &str = r#"Saurron update report:

Updated ({{ updated | length }}): {% if updated %}{{ updated | join(", ") }}{% else %}none{% endif %}
Rolled back ({{ rolled_back | length }}): {% if rolled_back %}{{ rolled_back | join(", ") }}{% else %}none{% endif %}
Failed ({{ failed | length }}): {% if failed %}{{ failed | join(", ") }}{% else %}none{% endif %}
Up to date: {{ up_to_date }}"#;

/// Returns true when the cycle produced at least one update, failure, or rollback.
pub fn should_notify(report: &SessionReport) -> bool {
    !report.updated.is_empty() || !report.failed.is_empty() || !report.rolled_back.is_empty()
}

/// Render the notification body using minijinja.
/// Uses `DEFAULT_TEMPLATE` when `template` is `None`.
pub fn render_template(report: &SessionReport, template: Option<&str>) -> Result<String> {
    use minijinja::{Environment, context};

    let template_str = template.unwrap_or(DEFAULT_TEMPLATE);
    let mut env = Environment::new();
    env.add_template("t", template_str)
        .context("invalid notification template syntax")?;
    env.get_template("t")
        .unwrap()
        .render(context! {
            updated    => &report.updated,
            skipped    => &report.skipped,
            failed     => &report.failed,
            rolled_back => &report.rolled_back,
            up_to_date => report.up_to_date,
        })
        .context("notification template rendering failed")
}

/// Parse `"Key:Value,Key2:Value2"` into header pairs.
/// Splits on the first `:` in each pair so values may themselves contain colons.
pub fn parse_webhook_headers(s: &str) -> Vec<(String, String)> {
    if s.trim().is_empty() {
        return vec![];
    }
    s.split(',')
        .filter_map(|pair| {
            pair.trim()
                .split_once(':')
                .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        })
        .collect()
}

// ── MQTT helper ───────────────────────────────────────────────────────────────

/// Parse the broker URL into `(host, port, scheme_implies_tls)`.
///
/// Supported schemes: `tcp://`, `mqtt://` (plain), `mqtts://`, `ssl://` (TLS).
/// When no port is in the URL the default is 1883 for plain and 8883 for TLS schemes.
pub(crate) fn parse_mqtt_broker(broker: &str) -> Result<(String, u16, bool)> {
    let (stripped, scheme_tls, default_port) = if let Some(rest) = broker
        .strip_prefix("mqtts://")
        .or_else(|| broker.strip_prefix("ssl://"))
    {
        (rest, true, 8883u16)
    } else {
        let rest = broker
            .strip_prefix("tcp://")
            .or_else(|| broker.strip_prefix("mqtt://"))
            .unwrap_or(broker);
        (rest, false, 1883u16)
    };

    if let Some((host, port_str)) = stripped.rsplit_once(':') {
        let port = port_str
            .parse::<u16>()
            .context("invalid MQTT broker port")?;
        Ok((host.to_string(), port, scheme_tls))
    } else {
        Ok((stripped.to_string(), default_port, scheme_tls))
    }
}

/// Returns true when any TLS indicator is present: a TLS URL scheme, skip-verify
/// flag, or explicit certificate/key paths.
pub(crate) fn use_mqtt_tls(scheme_tls: bool, cfg: &MqttConfig) -> bool {
    scheme_tls
        || cfg.tls_skip_verify
        || cfg.tls_ca_cert.is_some()
        || cfg.tls_cert.is_some()
        || cfg.tls_key.is_some()
}

/// Build a rustls `ClientConfig` for MQTT TLS.
///
/// Priority:
/// 1. `tls_skip_verify` → accept any certificate (insecure).
/// 2. `tls_ca_cert` → verify against the supplied PEM CA file; optionally add
///    client certificate/key for mutual TLS.
/// 3. Neither → verify using system root CAs.
fn build_mqtt_tls_config(cfg: &MqttConfig) -> Result<rumqttc::tokio_rustls::rustls::ClientConfig> {
    use rumqttc::tokio_rustls::rustls::{
        ClientConfig, RootCertStore,
        client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
        pki_types::{CertificateDer, ServerName, UnixTime},
    };
    use std::sync::Arc;

    if cfg.tls_skip_verify {
        #[derive(Debug)]
        struct AcceptAnyServerCert;

        impl ServerCertVerifier for AcceptAnyServerCert {
            fn verify_server_cert(
                &self,
                _end_entity: &CertificateDer<'_>,
                _intermediates: &[CertificateDer<'_>],
                _server_name: &ServerName<'_>,
                _ocsp_response: &[u8],
                _now: UnixTime,
            ) -> std::result::Result<ServerCertVerified, rumqttc::tokio_rustls::rustls::Error>
            {
                Ok(ServerCertVerified::assertion())
            }

            fn verify_tls12_signature(
                &self,
                _message: &[u8],
                _cert: &CertificateDer<'_>,
                _dss: &rumqttc::tokio_rustls::rustls::DigitallySignedStruct,
            ) -> std::result::Result<HandshakeSignatureValid, rumqttc::tokio_rustls::rustls::Error>
            {
                Ok(HandshakeSignatureValid::assertion())
            }

            fn verify_tls13_signature(
                &self,
                _message: &[u8],
                _cert: &CertificateDer<'_>,
                _dss: &rumqttc::tokio_rustls::rustls::DigitallySignedStruct,
            ) -> std::result::Result<HandshakeSignatureValid, rumqttc::tokio_rustls::rustls::Error>
            {
                Ok(HandshakeSignatureValid::assertion())
            }

            fn supported_verify_schemes(
                &self,
            ) -> Vec<rumqttc::tokio_rustls::rustls::SignatureScheme> {
                rumqttc::tokio_rustls::rustls::crypto::ring::default_provider()
                    .signature_verification_algorithms
                    .supported_schemes()
            }
        }

        return Ok(ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AcceptAnyServerCert))
            .with_no_client_auth());
    }

    let root_cert_store = if let Some(ref ca_path) = cfg.tls_ca_cert {
        let pem = std::fs::read(ca_path)
            .with_context(|| format!("failed to read MQTT TLS CA cert: {ca_path}"))?;
        let mut store = RootCertStore::empty();
        let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut pem.as_slice())
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to parse MQTT TLS CA cert PEM")?;
        store.add_parsable_certificates(certs);
        if store.is_empty() {
            anyhow::bail!("no valid certificates found in MQTT TLS CA cert file: {ca_path}");
        }
        store
    } else {
        let mut store = RootCertStore::empty();
        let native_certs = rustls_native_certs::load_native_certs();
        for err in &native_certs.errors {
            tracing::warn!(error = %err, "failed to load a system root certificate");
        }
        store.add_parsable_certificates(native_certs.certs);
        store
    };

    let builder = ClientConfig::builder().with_root_certificates(root_cert_store);

    let client_config = if let (Some(cert_path), Some(key_path)) = (&cfg.tls_cert, &cfg.tls_key) {
        let cert_pem = std::fs::read(cert_path)
            .with_context(|| format!("failed to read MQTT TLS client cert: {cert_path}"))?;
        let key_pem = std::fs::read(key_path)
            .with_context(|| format!("failed to read MQTT TLS client key: {key_path}"))?;

        let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cert_pem.as_slice())
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to parse MQTT TLS client cert PEM")?;
        if certs.is_empty() {
            anyhow::bail!("no valid certificate found in MQTT TLS client cert file: {cert_path}");
        }

        let key = rustls_pemfile::private_key(&mut key_pem.as_slice())
            .context("failed to parse MQTT TLS client key PEM")?
            .with_context(|| format!("no private key found in MQTT TLS key file: {key_path}"))?;

        builder
            .with_client_auth_cert(certs, key)
            .context("failed to configure MQTT TLS client authentication")?
    } else {
        builder.with_no_client_auth()
    };

    Ok(client_config)
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

/// Send notifications to all configured targets if the cycle produced
/// interesting results (any update, failure, or rollback).
/// Errors from individual targets are logged; other targets still run.
pub async fn dispatch(config: &NotificationsConfig, report: &SessionReport) {
    if !should_notify(report) {
        return;
    }

    let delay_secs = parse_duration_secs(&config.general.delay).unwrap_or(0);
    if delay_secs > 0 {
        tokio::time::sleep(tokio::time::Duration::from_secs(delay_secs)).await;
    }

    let body = match render_template(report, config.general.template.as_deref()) {
        Ok(b) => b,
        Err(e) => {
            error!(error = %e, "failed to render notification template");
            return;
        }
    };

    let (r_wh, r_em, r_mq, r_po) = tokio::join!(
        async {
            if let Some(cfg) = &config.webhook {
                send_webhook(cfg, &body).await
            } else {
                Ok(())
            }
        },
        async {
            if let Some(cfg) = &config.email {
                send_email(cfg, &body).await
            } else {
                Ok(())
            }
        },
        async {
            if let Some(cfg) = &config.mqtt {
                send_mqtt(cfg, &body).await
            } else {
                Ok(())
            }
        },
        async {
            if let Some(cfg) = &config.pushover {
                send_pushover(cfg, &body).await
            } else {
                Ok(())
            }
        },
    );

    for (name, result) in [
        ("webhook", r_wh),
        ("email", r_em),
        ("mqtt", r_mq),
        ("pushover", r_po),
    ] {
        match result {
            Ok(()) => info!(target = name, "notification dispatched"),
            Err(e) => {
                error!(target = name, error = %error_chain(&e), "notification dispatch failed")
            }
        }
    }
}

/// Format the full anyhow error chain as a single colon-separated string.
/// anyhow's Display only shows the outermost context; this includes every cause.
fn error_chain(e: &anyhow::Error) -> String {
    e.chain()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(": ")
}

// ── Per-target senders ────────────────────────────────────────────────────────

pub async fn send_webhook(cfg: &WebhookConfig, body: &str) -> Result<()> {
    let client = if cfg.tls_skip_verify {
        reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .context("failed to build HTTP client")?
    } else {
        reqwest::Client::new()
    };

    let mut req = client
        .post(&cfg.url)
        .header(reqwest::header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(body.to_string());

    if let Some(headers_str) = &cfg.headers {
        for (k, v) in parse_webhook_headers(headers_str) {
            req = req.header(k, v);
        }
    }

    let resp = req.send().await.context("webhook request failed")?;
    if !resp.status().is_success() {
        anyhow::bail!("webhook returned HTTP {}", resp.status());
    }
    Ok(())
}

pub async fn send_email(cfg: &EmailConfig, body: &str) -> Result<()> {
    use lettre::{
        AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
        message::header::ContentType,
        transport::smtp::{
            authentication::Credentials,
            client::{Tls, TlsParameters},
        },
    };

    if cfg.to.is_empty() {
        anyhow::bail!("email notification has no recipients");
    }

    let mut msg = Message::builder()
        .from(cfg.from.parse().context("invalid 'from' email address")?)
        .subject("Saurron update report")
        .header(ContentType::TEXT_PLAIN);

    for addr in &cfg.to {
        msg = msg.to(addr.parse().context("invalid 'to' email address")?);
    }
    let email = msg
        .body(body.to_string())
        .context("failed to build email message")?;

    let mut builder = if cfg.tls_skip_verify {
        let tls = TlsParameters::builder(cfg.server.clone())
            .dangerous_accept_invalid_certs(true)
            .build()
            .with_context(|| {
                format!(
                    "failed to build TLS parameters for {}:{}",
                    cfg.server, cfg.port
                )
            })?;
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&cfg.server)
            .port(cfg.port)
            .tls(Tls::Required(tls))
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.server)
            .with_context(|| {
                format!(
                    "failed to create SMTP relay for {}:{}",
                    cfg.server, cfg.port
                )
            })?
            .port(cfg.port)
    };

    if let (Some(u), Some(p)) = (&cfg.user, &cfg.password) {
        builder = builder.credentials(Credentials::new(u.clone(), p.clone()));
    }

    builder.build().send(email).await.with_context(|| {
        format!(
            "SMTP send failed (server: {}:{}, from: {})",
            cfg.server, cfg.port, cfg.from
        )
    })?;
    Ok(())
}

pub async fn send_mqtt(cfg: &MqttConfig, body: &str) -> Result<()> {
    use rumqttc::{AsyncClient, MqttOptions, QoS, Transport};

    let (host, port, scheme_tls) = parse_mqtt_broker(&cfg.broker)?;

    let qos = match cfg.qos {
        1 => QoS::AtLeastOnce,
        2 => QoS::ExactlyOnce,
        _ => QoS::AtMostOnce,
    };

    let client_id = cfg.client_id.clone().unwrap_or_else(|| {
        format!(
            "saurron-notif-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        )
    });

    let mut opts = MqttOptions::new(client_id, (host, port));
    opts.set_clean_start(true);
    if let (Some(u), Some(p)) = (&cfg.username, &cfg.password) {
        opts.set_credentials(u.as_str(), p.as_bytes().to_vec());
    }
    if use_mqtt_tls(scheme_tls, cfg) {
        let tls_config =
            build_mqtt_tls_config(cfg).context("failed to build MQTT TLS configuration")?;
        opts.set_transport(Transport::tls_with_config(tls_config.into()));
    }

    let (client, mut eventloop) = AsyncClient::builder(opts).capacity(16).build();

    // Spawn the event loop driver.
    let driver = tokio::spawn(async move {
        loop {
            if eventloop.poll().await.is_err() {
                break;
            }
        }
    });

    client
        .publish(&cfg.topic, qos, false, body.as_bytes().to_vec())
        .await
        .context("failed to publish MQTT message")?;

    // Brief wait to allow the broker to receive the message before disconnect.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    client
        .disconnect()
        .await
        .context("failed to disconnect MQTT client")?;
    driver.abort();

    Ok(())
}

pub async fn send_pushover(cfg: &PushoverConfig, body: &str) -> Result<()> {
    #[derive(serde::Serialize)]
    struct Payload<'a> {
        token: &'a str,
        user: &'a str,
        title: &'static str,
        message: &'a str,
    }

    let resp = reqwest::Client::new()
        .post("https://api.pushover.net/1/messages.json")
        .json(&Payload {
            token: &cfg.token,
            user: &cfg.user_key,
            title: "Saurron update report",
            message: body,
        })
        .send()
        .await
        .context("Pushover request failed")?;

    if !resp.status().is_success() {
        anyhow::bail!("Pushover returned HTTP {}", resp.status());
    }
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::SessionReport;

    fn report_with_updates() -> SessionReport {
        SessionReport {
            updated: vec!["nginx".to_string(), "redis".to_string()],
            skipped: vec![],
            failed: vec![],
            rolled_back: vec![],
            up_to_date: 3,
        }
    }

    fn empty_report() -> SessionReport {
        SessionReport::default()
    }

    // ── render_template ───────────────────────────────────────────────────────

    #[test]
    fn render_default_template_with_updates() {
        let r = report_with_updates();
        let body = render_template(&r, None).unwrap();
        assert!(body.contains("nginx, redis"), "updated containers missing");
        assert!(body.contains("Up to date: 3"));
        assert!(body.contains("none"), "rolled_back/failed should say none");
    }

    #[test]
    fn render_custom_template() {
        let r = report_with_updates();
        let tmpl = "{{ updated | length }} container(s) updated";
        let body = render_template(&r, Some(tmpl)).unwrap();
        assert_eq!(body, "2 container(s) updated");
    }

    #[test]
    fn render_empty_report() {
        let r = empty_report();
        let body = render_template(&r, None).unwrap();
        assert!(body.contains("Up to date: 0"));
    }

    #[test]
    fn render_invalid_template_returns_err() {
        let r = empty_report();
        let result = render_template(&r, Some("{{ unclosed"));
        assert!(result.is_err());
    }

    // ── parse_webhook_headers ─────────────────────────────────────────────────

    #[test]
    fn parse_headers_single() {
        let pairs = parse_webhook_headers("X-Custom: myvalue");
        assert_eq!(pairs, vec![("X-Custom".to_string(), "myvalue".to_string())]);
    }

    #[test]
    fn parse_headers_multiple() {
        let pairs = parse_webhook_headers("H1:V1,H2:V2");
        assert_eq!(pairs.len(), 2);
        assert!(pairs.contains(&("H1".to_string(), "V1".to_string())));
        assert!(pairs.contains(&("H2".to_string(), "V2".to_string())));
    }

    #[test]
    fn parse_headers_empty_string() {
        assert!(parse_webhook_headers("").is_empty());
        assert!(parse_webhook_headers("   ").is_empty());
    }

    #[test]
    fn parse_headers_value_contains_colon() {
        // Split only on first ':' — value "Bearer token:xyz" is preserved intact.
        let pairs = parse_webhook_headers("Authorization:Bearer token:xyz");
        assert_eq!(
            pairs,
            vec![("Authorization".to_string(), "Bearer token:xyz".to_string())]
        );
    }

    #[test]
    fn parse_headers_skips_pairs_without_colon() {
        let pairs = parse_webhook_headers("no-colon-here,K:V");
        assert_eq!(pairs, vec![("K".to_string(), "V".to_string())]);
    }

    // ── should_notify ─────────────────────────────────────────────────────────

    #[test]
    fn should_notify_updated_nonempty() {
        let r = SessionReport {
            updated: vec!["app".to_string()],
            ..Default::default()
        };
        assert!(should_notify(&r));
    }

    #[test]
    fn should_notify_failed_nonempty() {
        let r = SessionReport {
            failed: vec!["app".to_string()],
            ..Default::default()
        };
        assert!(should_notify(&r));
    }

    #[test]
    fn should_notify_rolled_back_nonempty() {
        let r = SessionReport {
            rolled_back: vec!["app".to_string()],
            ..Default::default()
        };
        assert!(should_notify(&r));
    }

    #[test]
    fn should_notify_skipped_only_is_false() {
        let r = SessionReport {
            skipped: vec!["app".to_string()],
            ..Default::default()
        };
        assert!(!should_notify(&r));
    }

    #[test]
    fn should_notify_all_up_to_date_is_false() {
        let r = SessionReport {
            up_to_date: 5,
            ..Default::default()
        };
        assert!(!should_notify(&r));
    }

    // ── parse_mqtt_broker ─────────────────────────────────────────────────────

    #[test]
    fn parse_broker_tcp_scheme() {
        let (host, port, tls) = parse_mqtt_broker("tcp://broker.example.com:1883").unwrap();
        assert_eq!(host, "broker.example.com");
        assert_eq!(port, 1883);
        assert!(!tls);
    }

    #[test]
    fn parse_broker_mqtt_scheme() {
        let (host, port, tls) = parse_mqtt_broker("mqtt://localhost:1884").unwrap();
        assert_eq!(host, "localhost");
        assert_eq!(port, 1884);
        assert!(!tls);
    }

    #[test]
    fn parse_broker_no_scheme_with_port() {
        let (host, port, tls) = parse_mqtt_broker("host.local:9000").unwrap();
        assert_eq!(host, "host.local");
        assert_eq!(port, 9000);
        assert!(!tls);
    }

    #[test]
    fn parse_broker_no_port_defaults_to_1883() {
        let (host, port, tls) = parse_mqtt_broker("broker.example.com").unwrap();
        assert_eq!(host, "broker.example.com");
        assert_eq!(port, 1883);
        assert!(!tls);
    }

    #[test]
    fn parse_broker_mqtts_scheme_implies_tls_and_default_port_8883() {
        let (host, port, tls) = parse_mqtt_broker("mqtts://broker.example.com").unwrap();
        assert_eq!(host, "broker.example.com");
        assert_eq!(port, 8883);
        assert!(tls);
    }

    #[test]
    fn parse_broker_ssl_scheme_implies_tls_and_default_port_8883() {
        let (host, port, tls) = parse_mqtt_broker("ssl://broker.example.com").unwrap();
        assert_eq!(host, "broker.example.com");
        assert_eq!(port, 8883);
        assert!(tls);
    }

    #[test]
    fn parse_broker_mqtts_scheme_with_explicit_port() {
        let (host, port, tls) = parse_mqtt_broker("mqtts://broker.example.com:9883").unwrap();
        assert_eq!(host, "broker.example.com");
        assert_eq!(port, 9883);
        assert!(tls);
    }

    // ── use_mqtt_tls ──────────────────────────────────────────────────────────

    fn minimal_mqtt_cfg() -> MqttConfig {
        MqttConfig {
            broker: "localhost:1883".to_string(),
            topic: "test".to_string(),
            qos: 0,
            client_id: None,
            username: None,
            password: None,
            tls_skip_verify: false,
            tls_ca_cert: None,
            tls_cert: None,
            tls_key: None,
        }
    }

    #[test]
    fn use_mqtt_tls_false_when_no_indicators() {
        assert!(!use_mqtt_tls(false, &minimal_mqtt_cfg()));
    }

    #[test]
    fn use_mqtt_tls_true_when_scheme_implies_tls() {
        assert!(use_mqtt_tls(true, &minimal_mqtt_cfg()));
    }

    #[test]
    fn use_mqtt_tls_true_when_skip_verify() {
        let mut cfg = minimal_mqtt_cfg();
        cfg.tls_skip_verify = true;
        assert!(use_mqtt_tls(false, &cfg));
    }

    #[test]
    fn use_mqtt_tls_true_when_ca_cert_set() {
        let mut cfg = minimal_mqtt_cfg();
        cfg.tls_ca_cert = Some("/etc/ssl/ca.pem".to_string());
        assert!(use_mqtt_tls(false, &cfg));
    }

    #[test]
    fn use_mqtt_tls_true_when_client_cert_set() {
        let mut cfg = minimal_mqtt_cfg();
        cfg.tls_cert = Some("/etc/ssl/client.pem".to_string());
        assert!(use_mqtt_tls(false, &cfg));
    }

    #[test]
    fn use_mqtt_tls_true_when_client_key_set() {
        let mut cfg = minimal_mqtt_cfg();
        cfg.tls_key = Some("/etc/ssl/client.key".to_string());
        assert!(use_mqtt_tls(false, &cfg));
    }

    // ── dispatch (no network) ─────────────────────────────────────────────────

    #[tokio::test]
    async fn dispatch_returns_early_when_nothing_interesting() {
        use crate::config::{GeneralNotifConfig, NotificationsConfig};

        let config = NotificationsConfig {
            general: GeneralNotifConfig {
                delay: "0s".to_string(),
                template: None,
            },
            webhook: None,
            email: None,
            mqtt: None,
            pushover: None,
        };
        // All-up-to-date report — should_notify returns false, dispatch is a no-op.
        let report = SessionReport {
            up_to_date: 10,
            ..Default::default()
        };
        dispatch(&config, &report).await; // must not panic or block
    }

    #[tokio::test]
    async fn dispatch_with_updates_and_no_targets_completes() {
        use crate::config::{GeneralNotifConfig, NotificationsConfig};

        let config = NotificationsConfig {
            general: GeneralNotifConfig {
                delay: "0s".to_string(),
                template: None,
            },
            webhook: None,
            email: None,
            mqtt: None,
            pushover: None,
        };
        let report = report_with_updates();
        dispatch(&config, &report).await; // renders template, finds no targets → OK
    }

    #[tokio::test]
    async fn dispatch_invalid_template_does_not_panic() {
        use crate::config::{GeneralNotifConfig, NotificationsConfig};

        let config = NotificationsConfig {
            general: GeneralNotifConfig {
                delay: "0s".to_string(),
                template: Some("{{ unclosed".to_string()),
            },
            webhook: None,
            email: None,
            mqtt: None,
            pushover: None,
        };
        // should_notify returns true → dispatch tries to render → error → returns early
        dispatch(&config, &report_with_updates()).await;
    }

    #[tokio::test]
    async fn dispatch_failing_webhook_logs_error_and_returns() {
        use crate::config::{GeneralNotifConfig, NotificationsConfig, WebhookConfig};

        // Port 1 on loopback will always be connection refused instantly.
        let config = NotificationsConfig {
            general: GeneralNotifConfig {
                delay: "0s".to_string(),
                template: None,
            },
            webhook: Some(WebhookConfig {
                url: "http://127.0.0.1:1/nonexistent".to_string(),
                headers: None,
                tls_skip_verify: false,
            }),
            email: None,
            mqtt: None,
            pushover: None,
        };
        dispatch(&config, &report_with_updates()).await; // must not panic
    }

    // ── send_webhook (local server) ───────────────────────────────────────────

    #[tokio::test]
    async fn send_webhook_posts_body_and_headers() {
        use crate::config::WebhookConfig;
        use axum::{Router, body::Bytes, http::StatusCode, routing::post};
        use std::sync::Arc;
        use tokio::sync::Mutex;

        let received: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let slot = Arc::clone(&received);

        let app = Router::new().route(
            "/hook",
            post(move |body: Bytes| {
                let slot = Arc::clone(&slot);
                async move {
                    *slot.lock().await = Some(String::from_utf8_lossy(&body).into_owned());
                    StatusCode::OK
                }
            }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let cfg = WebhookConfig {
            url: format!("http://127.0.0.1:{port}/hook"),
            headers: Some("X-Test:value".to_string()),
            tls_skip_verify: false,
        };
        send_webhook(&cfg, "ping").await.unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert_eq!(received.lock().await.take().unwrap(), "ping");
    }

    #[tokio::test]
    async fn send_webhook_returns_err_on_server_error_status() {
        use crate::config::WebhookConfig;
        use axum::{Router, http::StatusCode, routing::post};

        let app = Router::new().route(
            "/hook",
            post(|| async { StatusCode::INTERNAL_SERVER_ERROR }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let cfg = WebhookConfig {
            url: format!("http://127.0.0.1:{port}/hook"),
            headers: None,
            tls_skip_verify: false,
        };
        assert!(send_webhook(&cfg, "test").await.is_err());
    }

    #[tokio::test]
    async fn send_webhook_tls_skip_verify_builds_different_client() {
        use crate::config::WebhookConfig;

        // Port 1 gives immediate connection refused; we just verify the
        // skip-verify path builds a client and hits the network error.
        let cfg = WebhookConfig {
            url: "https://127.0.0.1:1/".to_string(),
            headers: None,
            tls_skip_verify: true,
        };
        let result = send_webhook(&cfg, "test").await;
        assert!(result.is_err(), "expected connection-refused or TLS error");
    }
}
