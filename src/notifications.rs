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

/// Parse `tcp://host:port`, `mqtt://host:port`, or bare `host:port` / `host`.
pub(crate) fn parse_mqtt_broker(broker: &str) -> Result<(String, u16)> {
    let stripped = broker
        .strip_prefix("tcp://")
        .or_else(|| broker.strip_prefix("mqtt://"))
        .unwrap_or(broker);

    if let Some((host, port_str)) = stripped.rsplit_once(':') {
        let port = port_str
            .parse::<u16>()
            .context("invalid MQTT broker port")?;
        Ok((host.to_string(), port))
    } else {
        Ok((stripped.to_string(), 1883))
    }
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
            Err(e) => error!(target = name, error = %e, "notification dispatch failed"),
        }
    }
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
            .context("failed to build TLS parameters")?;
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&cfg.server)
            .port(cfg.port)
            .tls(Tls::Required(tls))
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.server)
            .context("failed to create SMTP relay")?
            .port(cfg.port)
    };

    if let (Some(u), Some(p)) = (&cfg.user, &cfg.password) {
        builder = builder.credentials(Credentials::new(u.clone(), p.clone()));
    }

    builder
        .build()
        .send(email)
        .await
        .context("SMTP send failed")?;
    Ok(())
}

pub async fn send_mqtt(cfg: &MqttConfig, body: &str) -> Result<()> {
    use rumqttc::{AsyncClient, MqttOptions, QoS};

    let (host, port) = parse_mqtt_broker(&cfg.broker)?;

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

    let (client, mut eventloop) = AsyncClient::new(opts, 16);

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
        let (host, port) = parse_mqtt_broker("tcp://broker.example.com:1883").unwrap();
        assert_eq!(host, "broker.example.com");
        assert_eq!(port, 1883);
    }

    #[test]
    fn parse_broker_mqtt_scheme() {
        let (host, port) = parse_mqtt_broker("mqtt://localhost:1884").unwrap();
        assert_eq!(host, "localhost");
        assert_eq!(port, 1884);
    }

    #[test]
    fn parse_broker_no_scheme_with_port() {
        let (host, port) = parse_mqtt_broker("host.local:9000").unwrap();
        assert_eq!(host, "host.local");
        assert_eq!(port, 9000);
    }

    #[test]
    fn parse_broker_no_port_defaults_to_1883() {
        let (host, port) = parse_mqtt_broker("broker.example.com").unwrap();
        assert_eq!(host, "broker.example.com");
        assert_eq!(port, 1883);
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
