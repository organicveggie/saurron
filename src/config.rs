use anyhow::{Context, Result};
use serde::Deserialize;

use crate::cli::{Args, HeadWarnStrategy, LogFormat, LogLevel};

// ── Concrete domain types ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Config {
    pub log_level: LogLevel,
    pub log_format: LogFormat,
    pub audit_log: Option<String>,
    pub poll_interval: Option<String>,
    pub schedule: Option<String>,
    pub run_once: bool,
    pub docker: DockerConfig,
    pub label_enable: bool,
    pub disable_containers: Vec<String>,
    pub containers: Vec<String>,
    pub include_restarting: bool,
    pub global_takes_precedence: bool,
    pub monitor_only: bool,
    pub no_pull: bool,
    pub cleanup: bool,
    pub revive_stopped: bool,
    pub stop_timeout: String,
    pub rollback: RollbackConfig,
    pub head_warn_strategy: HeadWarnStrategy,
    pub registry_username: Option<String>,
    pub registry_password: Option<String>,
    pub http_api: HttpApiConfig,
    pub notifications: NotificationsConfig,
}

#[derive(Debug, Clone)]
pub struct DockerConfig {
    pub host: String,
    pub tls_verify: bool,
    pub tls_ca_cert: Option<String>,
    pub tls_cert: Option<String>,
    pub tls_key: Option<String>,
    pub api_version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RollbackConfig {
    pub on_exit_code: bool,
    pub on_healthcheck: bool,
    pub on_timeout: bool,
    pub startup_timeout: String,
}

#[derive(Debug, Clone)]
pub struct HttpApiConfig {
    pub update: bool,
    pub metrics: bool,
    pub token: Option<String>,
    pub port: u16,
    pub metrics_no_auth: bool,
}

#[derive(Debug, Clone)]
pub struct NotificationsConfig {
    pub general: GeneralNotifConfig,
    pub webhook: Option<WebhookConfig>,
    pub email: Option<EmailConfig>,
    pub mqtt: Option<MqttConfig>,
    pub pushover: Option<PushoverConfig>,
}

#[derive(Debug, Clone)]
pub struct GeneralNotifConfig {
    pub delay: String,
    pub template: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WebhookConfig {
    pub url: String,
    pub headers: Option<String>,
    pub tls_skip_verify: bool,
}

#[derive(Debug, Clone)]
pub struct EmailConfig {
    pub from: String,
    pub to: Vec<String>,
    pub server: String,
    pub port: u16,
    pub user: Option<String>,
    pub password: Option<String>,
    pub tls_skip_verify: bool,
}

#[derive(Debug, Clone)]
pub struct MqttConfig {
    pub broker: String,
    pub topic: String,
    pub qos: u8,
    pub client_id: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PushoverConfig {
    pub token: String,
    pub user_key: String,
}

// ── TOML-deserializable partial types (all fields Optional) ──────────────────

#[derive(Debug, Default, Deserialize)]
struct PartialConfig {
    log_level: Option<LogLevel>,
    log_format: Option<LogFormat>,
    audit_log: Option<String>,
    poll_interval: Option<String>,
    schedule: Option<String>,
    run_once: Option<bool>,
    label_enable: Option<bool>,
    disable_containers: Option<Vec<String>>,
    containers: Option<Vec<String>>,
    include_restarting: Option<bool>,
    global_takes_precedence: Option<bool>,
    monitor_only: Option<bool>,
    no_pull: Option<bool>,
    cleanup: Option<bool>,
    revive_stopped: Option<bool>,
    stop_timeout: Option<String>,
    head_warn_strategy: Option<HeadWarnStrategy>,
    registry_username: Option<String>,
    registry_password: Option<String>,
    docker: Option<PartialDockerConfig>,
    rollback: Option<PartialRollbackConfig>,
    http_api: Option<PartialHttpApiConfig>,
    notifications: Option<PartialNotificationsConfig>,
}

#[derive(Debug, Default, Deserialize)]
struct PartialDockerConfig {
    host: Option<String>,
    tls_verify: Option<bool>,
    tls_ca_cert: Option<String>,
    tls_cert: Option<String>,
    tls_key: Option<String>,
    api_version: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct PartialRollbackConfig {
    on_exit_code: Option<bool>,
    on_healthcheck: Option<bool>,
    on_timeout: Option<bool>,
    startup_timeout: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct PartialHttpApiConfig {
    update: Option<bool>,
    metrics: Option<bool>,
    token: Option<String>,
    port: Option<u16>,
    metrics_no_auth: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct PartialNotificationsConfig {
    general: Option<PartialGeneralNotifConfig>,
    webhook: Option<PartialWebhookConfig>,
    email: Option<PartialEmailConfig>,
    mqtt: Option<PartialMqttConfig>,
    pushover: Option<PartialPushoverConfig>,
}

#[derive(Debug, Default, Deserialize)]
struct PartialGeneralNotifConfig {
    delay: Option<String>,
    template: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct PartialWebhookConfig {
    url: Option<String>,
    headers: Option<String>,
    tls_skip_verify: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct PartialEmailConfig {
    from: Option<String>,
    to: Option<Vec<String>>,
    server: Option<String>,
    port: Option<u16>,
    user: Option<String>,
    password: Option<String>,
    tls_skip_verify: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct PartialMqttConfig {
    broker: Option<String>,
    topic: Option<String>,
    qos: Option<u8>,
    client_id: Option<String>,
    username: Option<String>,
    password: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct PartialPushoverConfig {
    token: Option<String>,
    user_key: Option<String>,
}

// ── Config loading ────────────────────────────────────────────────────────────

impl Config {
    /// Load config from TOML file → env vars → CLI flags, with built-in defaults
    /// as the base layer. Secret file resolution applied after merge.
    pub fn load(args: &Args) -> Result<Self> {
        let config_path = args.config.as_deref().unwrap_or("/etc/saurron/config.toml");

        let partial: PartialConfig = config::Config::builder()
            .add_source(config::File::with_name(config_path).required(false))
            .build()
            .context("failed to build config")?
            .try_deserialize()
            .unwrap_or_default();

        let mut cfg = Self::merge(args, partial);
        cfg.resolve_secrets()?;
        Ok(cfg)
    }

    fn merge(args: &Args, p: PartialConfig) -> Self {
        let pd = p.docker.unwrap_or_default();
        let pr = p.rollback.unwrap_or_default();
        let ph = p.http_api.unwrap_or_default();
        let pn = p.notifications.unwrap_or_default();
        let pg = pn.general.unwrap_or_default();

        // Rollback: --no-rollback-on-* flags explicitly set to false
        let on_exit_code = if args.no_rollback_on_exit_code {
            false
        } else {
            args.rollback_on_exit_code
                .or(pr.on_exit_code)
                .unwrap_or(true)
        };
        let on_healthcheck = if args.no_rollback_on_healthcheck {
            false
        } else {
            args.rollback_on_healthcheck
                .or(pr.on_healthcheck)
                .unwrap_or(true)
        };
        let on_timeout = if args.no_rollback_on_timeout {
            false
        } else {
            args.rollback_on_timeout.or(pr.on_timeout).unwrap_or(true)
        };

        // Webhook: only present if url is configured
        let webhook = args
            .webhook_url
            .clone()
            .or(pn.webhook.as_ref().and_then(|w| w.url.clone()))
            .map(|url| {
                let pw = pn.webhook.as_ref();
                WebhookConfig {
                    url,
                    headers: args
                        .webhook_headers
                        .clone()
                        .or_else(|| pw.and_then(|w| w.headers.clone())),
                    tls_skip_verify: args
                        .webhook_tls_skip_verify
                        .or_else(|| pw.and_then(|w| w.tls_skip_verify))
                        .unwrap_or(false),
                }
            });

        // Email: only present if from + to + server are all configured
        let email = {
            let pe = pn.email.as_ref();
            let from = args
                .notification_email_from
                .clone()
                .or_else(|| pe.and_then(|e| e.from.clone()));
            let to = args
                .notification_email_to
                .clone()
                .or_else(|| pe.and_then(|e| e.to.clone()));
            let server = args
                .notification_email_server
                .clone()
                .or_else(|| pe.and_then(|e| e.server.clone()));

            match (from, to, server) {
                (Some(from), Some(to), Some(server)) => Some(EmailConfig {
                    from,
                    to,
                    server,
                    port: args
                        .notification_email_port
                        .or_else(|| pe.and_then(|e| e.port))
                        .unwrap_or(587),
                    user: args
                        .notification_email_user
                        .clone()
                        .or_else(|| pe.and_then(|e| e.user.clone())),
                    password: args
                        .notification_email_password
                        .clone()
                        .or_else(|| pe.and_then(|e| e.password.clone())),
                    tls_skip_verify: args
                        .notification_email_tls_skip_verify
                        .or_else(|| pe.and_then(|e| e.tls_skip_verify))
                        .unwrap_or(false),
                }),
                _ => None,
            }
        };

        // MQTT: only present if broker + topic are configured
        let mqtt = {
            let pm = pn.mqtt.as_ref();
            let broker = args
                .notification_mqtt_broker
                .clone()
                .or_else(|| pm.and_then(|m| m.broker.clone()));
            let topic = args
                .notification_mqtt_topic
                .clone()
                .or_else(|| pm.and_then(|m| m.topic.clone()));

            match (broker, topic) {
                (Some(broker), Some(topic)) => Some(MqttConfig {
                    broker,
                    topic,
                    qos: args
                        .notification_mqtt_qos
                        .or_else(|| pm.and_then(|m| m.qos))
                        .unwrap_or(0),
                    client_id: args
                        .notification_mqtt_client_id
                        .clone()
                        .or_else(|| pm.and_then(|m| m.client_id.clone())),
                    username: args
                        .notification_mqtt_username
                        .clone()
                        .or_else(|| pm.and_then(|m| m.username.clone())),
                    password: args
                        .notification_mqtt_password
                        .clone()
                        .or_else(|| pm.and_then(|m| m.password.clone())),
                }),
                _ => None,
            }
        };

        // Pushover: only present if both token + user_key are configured
        let pushover = {
            let pp = pn.pushover.as_ref();
            let token = args
                .notification_pushover_token
                .clone()
                .or_else(|| pp.and_then(|p| p.token.clone()));
            let user_key = args
                .notification_pushover_user_key
                .clone()
                .or_else(|| pp.and_then(|p| p.user_key.clone()));

            match (token, user_key) {
                (Some(token), Some(user_key)) => Some(PushoverConfig { token, user_key }),
                _ => None,
            }
        };

        Config {
            log_level: if args.trace {
                LogLevel::Trace
            } else if args.debug {
                LogLevel::Debug
            } else {
                args.log_level.or(p.log_level).unwrap_or(LogLevel::Info)
            },
            log_format: args.log_format.or(p.log_format).unwrap_or(LogFormat::Auto),
            audit_log: args.audit_log.clone().or(p.audit_log),
            poll_interval: args.interval.clone().or(p.poll_interval),
            schedule: args.schedule.clone().or(p.schedule),
            run_once: args.run_once.or(p.run_once).unwrap_or(false),
            docker: DockerConfig {
                host: args
                    .host
                    .clone()
                    .or(pd.host)
                    .unwrap_or_else(|| "unix:///var/run/docker.sock".to_string()),
                tls_verify: args.tlsverify.or(pd.tls_verify).unwrap_or(false),
                tls_ca_cert: args.tls_ca_cert.clone().or(pd.tls_ca_cert),
                tls_cert: args.tls_cert.clone().or(pd.tls_cert),
                tls_key: args.tls_key.clone().or(pd.tls_key),
                api_version: args.api_version.clone().or(pd.api_version),
            },
            label_enable: args.label_enable.or(p.label_enable).unwrap_or(false),
            disable_containers: args
                .disable_containers
                .clone()
                .or(p.disable_containers)
                .unwrap_or_default(),
            containers: args.containers.clone().or(p.containers).unwrap_or_default(),
            include_restarting: args
                .include_restarting
                .or(p.include_restarting)
                .unwrap_or(false),
            global_takes_precedence: args
                .global_takes_precedence
                .or(p.global_takes_precedence)
                .unwrap_or(false),
            monitor_only: args.monitor_only.or(p.monitor_only).unwrap_or(false),
            no_pull: args.no_pull.or(p.no_pull).unwrap_or(false),
            cleanup: args.cleanup.or(p.cleanup).unwrap_or(false),
            revive_stopped: args.revive_stopped.or(p.revive_stopped).unwrap_or(false),
            stop_timeout: args
                .stop_timeout
                .clone()
                .or(p.stop_timeout)
                .unwrap_or_else(|| "10s".to_string()),
            rollback: RollbackConfig {
                on_exit_code,
                on_healthcheck,
                on_timeout,
                startup_timeout: args
                    .startup_timeout
                    .clone()
                    .or(pr.startup_timeout)
                    .unwrap_or_else(|| "30s".to_string()),
            },
            head_warn_strategy: args
                .head_warn_strategy
                .or(p.head_warn_strategy)
                .unwrap_or(HeadWarnStrategy::Auto),
            registry_username: args.registry_username.clone().or(p.registry_username),
            registry_password: args.registry_password.clone().or(p.registry_password),
            http_api: HttpApiConfig {
                update: args.http_api_update.or(ph.update).unwrap_or(false),
                metrics: args.http_api_metrics.or(ph.metrics).unwrap_or(false),
                token: args.http_api_token.clone().or(ph.token),
                port: args.http_api_port.or(ph.port).unwrap_or(8080),
                metrics_no_auth: args
                    .http_api_metrics_no_auth
                    .or(ph.metrics_no_auth)
                    .unwrap_or(false),
            },
            notifications: NotificationsConfig {
                general: GeneralNotifConfig {
                    delay: args
                        .notification_delay
                        .clone()
                        .or(pg.delay)
                        .unwrap_or_else(|| "0s".to_string()),
                    template: args.notification_template.clone().or(pg.template),
                },
                webhook,
                email,
                mqtt,
                pushover,
            },
        }
    }

    /// Replace designated sensitive field values with file contents if the value
    /// is a readable file path (Docker secrets pattern).
    fn resolve_secrets(&mut self) -> Result<()> {
        if let Some(ref v) = self.registry_password.clone() {
            self.registry_password = Some(resolve_secret_file(v)?);
        }
        if let Some(ref v) = self.http_api.token.clone() {
            self.http_api.token = Some(resolve_secret_file(v)?);
        }
        if let Some(ref notif) = self.notifications.general.template.clone() {
            self.notifications.general.template = Some(resolve_secret_file(notif)?);
        }
        if let Some(ref mut wh) = self.notifications.webhook {
            wh.url = resolve_secret_file(&wh.url.clone())?;
            if let Some(ref v) = wh.headers.clone() {
                wh.headers = Some(resolve_secret_file(v)?);
            }
        }
        if let Some(ref mut email) = self.notifications.email {
            email.from = resolve_secret_file(&email.from.clone())?;
            for addr in email.to.iter_mut() {
                *addr = resolve_secret_file(&addr.clone())?;
            }
            email.server = resolve_secret_file(&email.server.clone())?;
            if let Some(ref v) = email.user.clone() {
                email.user = Some(resolve_secret_file(v)?);
            }
            if let Some(ref v) = email.password.clone() {
                email.password = Some(resolve_secret_file(v)?);
            }
        }
        if let Some(ref mut mqtt) = self.notifications.mqtt {
            mqtt.broker = resolve_secret_file(&mqtt.broker.clone())?;
            mqtt.topic = resolve_secret_file(&mqtt.topic.clone())?;
            if let Some(ref v) = mqtt.client_id.clone() {
                mqtt.client_id = Some(resolve_secret_file(v)?);
            }
            if let Some(ref v) = mqtt.username.clone() {
                mqtt.username = Some(resolve_secret_file(v)?);
            }
            if let Some(ref v) = mqtt.password.clone() {
                mqtt.password = Some(resolve_secret_file(v)?);
            }
        }
        if let Some(ref mut po) = self.notifications.pushover {
            po.token = resolve_secret_file(&po.token.clone())?;
            po.user_key = resolve_secret_file(&po.user_key.clone())?;
        }
        Ok(())
    }
}

/// If `value` is the path to a readable file, return its contents (trimmed).
/// Otherwise return the value unchanged. Enables Docker secrets.
fn resolve_secret_file(value: &str) -> Result<String> {
    let path = std::path::Path::new(value);
    if path.is_file() {
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read secret file: {value}"))?;
        Ok(contents.trim_end().to_string())
    } else {
        Ok(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Args;
    use clap::Parser;

    fn args(extra: &[&str]) -> Args {
        let mut cmd = vec!["saurron"];
        cmd.extend_from_slice(extra);
        Args::parse_from(cmd)
    }

    #[test]
    fn defaults_without_config_file() {
        let cfg = Config::load(&args(&[])).unwrap();
        assert_eq!(cfg.log_level, LogLevel::Info);
        assert_eq!(cfg.log_format, LogFormat::Auto);
        assert!(!cfg.monitor_only);
        assert!(!cfg.run_once);
        assert_eq!(cfg.docker.host, "unix:///var/run/docker.sock");
        assert!(cfg.rollback.on_exit_code);
        assert!(cfg.rollback.on_healthcheck);
        assert!(cfg.rollback.on_timeout);
        assert_eq!(cfg.rollback.startup_timeout, "30s");
        assert_eq!(cfg.http_api.port, 8080);
    }

    #[test]
    fn cli_debug_flag_sets_log_level() {
        let cfg = Config::load(&args(&["--debug"])).unwrap();
        assert_eq!(cfg.log_level, LogLevel::Debug);
    }

    #[test]
    fn cli_trace_flag_sets_log_level() {
        let cfg = Config::load(&args(&["--trace"])).unwrap();
        assert_eq!(cfg.log_level, LogLevel::Trace);
    }

    #[test]
    fn no_rollback_flags_override_defaults() {
        let cfg = Config::load(&args(&["--no-rollback-on-exit-code"])).unwrap();
        assert!(!cfg.rollback.on_exit_code);
        assert!(cfg.rollback.on_healthcheck);
    }

    #[test]
    fn run_once_flag() {
        let cfg = Config::load(&args(&["--run-once"])).unwrap();
        assert!(cfg.run_once);
    }

    #[test]
    fn monitor_only_flag() {
        let cfg = Config::load(&args(&["--monitor-only"])).unwrap();
        assert!(cfg.monitor_only);
    }

    #[test]
    fn no_pull_flag() {
        let cfg = Config::load(&args(&["--no-pull"])).unwrap();
        assert!(cfg.no_pull);
    }

    #[test]
    fn webhook_url_creates_webhook_config() {
        let cfg = Config::load(&args(&["--webhook-url", "https://example.com/hook"])).unwrap();
        let wh = cfg.notifications.webhook.expect("webhook should be Some");
        assert_eq!(wh.url, "https://example.com/hook");
        assert!(!wh.tls_skip_verify);
    }

    #[test]
    fn pushover_absent_without_both_fields() {
        let cfg = Config::load(&args(&["--notification-pushover-token", "tok123"])).unwrap();
        assert!(cfg.notifications.pushover.is_none());
    }

    #[test]
    fn resolve_secret_file_non_path_returns_literal() {
        let cfg =
            Config::load(&args(&["--registry-password", "plaintextpassword"])).unwrap();
        assert_eq!(cfg.registry_password, Some("plaintextpassword".to_string()));
    }
}
