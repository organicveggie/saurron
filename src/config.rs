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

/// Return a fully-commented sample TOML config string covering every supported
/// option with its default value. Uncommented lines use real defaults; optional
/// or disabled settings are commented out with example values.
///
/// The output is valid TOML and can be loaded directly by [`Config::load`].
pub fn generate_sample_config() -> String {
    r#"# Saurron configuration file
# Generate a fresh copy with: saurron --generate-config [FILE]

# Log verbosity: trace, debug, info, warn, error
log_level = "info"

# Log format: auto (detects TTY → pretty, pipe → logfmt), json, logfmt, pretty
log_format = "auto"

# Append-only JSON audit log (optional; omit to disable)
# audit_log = "/var/log/saurron/audit.log"

# How often to check for updates, e.g. "5m", "1h", "3600"
# Mutually exclusive with `schedule` and `run_once`.
# poll_interval = "24h"

# Cron expression for update schedule, e.g. "0 3 * * *"
# Mutually exclusive with `poll_interval` and `run_once`.
# schedule = ""

# Exit after a single update cycle instead of running continuously.
run_once = false

# Registry credentials (or path to a Docker secret file)
# registry_username = ""
# registry_password = ""

# How to handle a failed manifest HEAD request:
#   auto   — warn on unexpected errors, silent on auth failures
#   always — always warn
#   never  — always log at debug level
head_warn_strategy = "auto"

[docker]
# Docker daemon socket or TCP address
host = "unix:///var/run/docker.sock"
# Verify TLS certificates for TCP connections
tls_verify = false
# TLS certificate paths (required when tls_verify = true)
# tls_ca_cert = ""
# tls_cert = ""
# tls_key = ""
# Override the negotiated Docker API version, e.g. "1.44"
# api_version = ""

# ── Container selection ─────────────────────────────────────────────────────

# Opt-in mode: only update containers with the saurron.enable=true label.
label_enable = false

# Containers to always exclude from updates (TOML array of names).
disable_containers = []

# If non-empty, only update containers in this allow-list.
containers = []

# Include containers in the "restarting" state.
include_restarting = false

# When true, global settings take precedence over per-container saurron.* labels.
global_takes_precedence = false

# Also start stopped (exited/created) containers when a newer image is found.
revive_stopped = false

# ── Update behaviour ────────────────────────────────────────────────────────

# Detect stale images but never restart containers.
monitor_only = false

# Skip pulling the new image; use whatever is already cached locally.
no_pull = false

# Remove the old image after a successful update.
cleanup = false

# How long to wait for a container to stop gracefully before sending SIGKILL.
stop_timeout = "10s"

[rollback]
# Roll back if the new container exits with a non-zero code.
on_exit_code = true
# Roll back if the new container's Docker healthcheck reports unhealthy.
on_healthcheck = true
# Roll back if the container does not reach the running state within startup_timeout.
on_timeout = true
# How long to wait for the new container to become healthy before rolling back.
startup_timeout = "30s"

[http_api]
# Enable the POST /v1/update endpoint.
update = false
# Enable the GET /v1/metrics endpoint.
metrics = false
# Bearer token required for authenticated endpoints (or path to a Docker secret file).
# token = ""
# Port the HTTP server listens on.
port = 8080
# Allow unauthenticated access to GET /v1/metrics.
metrics_no_auth = false

[notifications]
# Delay between cycle completion and notification dispatch, e.g. "0s", "30s".
delay = "0s"
# Path to a custom MiniJinja notification template file (optional).
# template = ""

[notifications.webhook]
# HTTP endpoint to POST update reports to (enables webhook notifications).
# url = ""
# Extra request headers as comma-separated "Key: Value" pairs.
# headers = ""
# Skip TLS certificate verification for the webhook endpoint.
tls_skip_verify = false

[notifications.email]
# SMTP server hostname (required to enable email notifications).
# server = ""
# SMTP server port.
port = 587
# Sender address (required).
# from = ""
# Recipient addresses (required; TOML array).
# to = []
# SMTP credentials (optional).
# user = ""
# password = ""
# Skip TLS certificate verification for the SMTP connection.
tls_skip_verify = false

[notifications.mqtt]
# MQTT broker address, e.g. "tcp://broker.example.com:1883" (required to enable MQTT).
# broker = ""
# Topic to publish update reports to (required).
# topic = ""
# QoS level: 0 (at most once), 1 (at least once), 2 (exactly once).
qos = 0
# Client ID sent to the broker (auto-generated if omitted).
# client_id = ""
# Broker credentials (optional).
# username = ""
# password = ""

[notifications.pushover]
# Pushover application token (required to enable Pushover notifications).
# token = ""
# Pushover user or group key (required).
# user_key = ""
"#
    .to_string()
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
        let cfg = Config::load(&args(&["--registry-password", "plaintextpassword"])).unwrap();
        assert_eq!(cfg.registry_password, Some("plaintextpassword".to_string()));
    }

    #[test]
    fn no_rollback_on_healthcheck_flag() {
        let cfg = Config::load(&args(&["--no-rollback-on-healthcheck"])).unwrap();
        assert!(!cfg.rollback.on_healthcheck);
        assert!(cfg.rollback.on_exit_code);
        assert!(cfg.rollback.on_timeout);
    }

    #[test]
    fn no_rollback_on_timeout_flag() {
        let cfg = Config::load(&args(&["--no-rollback-on-timeout"])).unwrap();
        assert!(!cfg.rollback.on_timeout);
        assert!(cfg.rollback.on_exit_code);
        assert!(cfg.rollback.on_healthcheck);
    }

    #[test]
    fn email_config_with_all_required_fields() {
        let cfg = Config::load(&args(&[
            "--notification-email-from",
            "from@example.com",
            "--notification-email-to",
            "to@example.com",
            "--notification-email-server",
            "smtp.example.com",
        ]))
        .unwrap();
        let email = cfg.notifications.email.expect("email should be Some");
        assert_eq!(email.from, "from@example.com");
        assert_eq!(email.to, vec!["to@example.com".to_string()]);
        assert_eq!(email.server, "smtp.example.com");
        assert_eq!(email.port, 587);
        assert!(email.user.is_none());
        assert!(email.password.is_none());
        assert!(!email.tls_skip_verify);
    }

    #[test]
    fn email_absent_without_server() {
        let cfg = Config::load(&args(&[
            "--notification-email-from",
            "from@example.com",
            "--notification-email-to",
            "to@example.com",
        ]))
        .unwrap();
        assert!(cfg.notifications.email.is_none());
    }

    #[test]
    fn mqtt_config_with_broker_and_topic() {
        let cfg = Config::load(&args(&[
            "--notification-mqtt-broker",
            "tcp://broker.example.com:1883",
            "--notification-mqtt-topic",
            "saurron/updates",
        ]))
        .unwrap();
        let mqtt = cfg.notifications.mqtt.expect("mqtt should be Some");
        assert_eq!(mqtt.broker, "tcp://broker.example.com:1883");
        assert_eq!(mqtt.topic, "saurron/updates");
        assert_eq!(mqtt.qos, 0);
        assert!(mqtt.client_id.is_none());
        assert!(mqtt.username.is_none());
        assert!(mqtt.password.is_none());
    }

    #[test]
    fn mqtt_absent_without_topic() {
        let cfg = Config::load(&args(&[
            "--notification-mqtt-broker",
            "tcp://broker.example.com:1883",
        ]))
        .unwrap();
        assert!(cfg.notifications.mqtt.is_none());
    }

    #[test]
    fn pushover_config_with_both_fields() {
        let cfg = Config::load(&args(&[
            "--notification-pushover-token",
            "tok123",
            "--notification-pushover-user-key",
            "user456",
        ]))
        .unwrap();
        let po = cfg.notifications.pushover.expect("pushover should be Some");
        assert_eq!(po.token, "tok123");
        assert_eq!(po.user_key, "user456");
    }

    #[test]
    fn webhook_secret_file_resolution() {
        let path = std::env::temp_dir().join("saurron_test_webhook_url.txt");
        std::fs::write(&path, "https://secret-hook.example.com/hook").unwrap();
        let path_str = path.to_str().unwrap().to_string();
        let cfg = Config::load(&args(&["--webhook-url", &path_str])).unwrap();
        std::fs::remove_file(&path).ok();
        let wh = cfg.notifications.webhook.expect("webhook should be Some");
        assert_eq!(wh.url, "https://secret-hook.example.com/hook");
    }

    #[test]
    fn http_api_token_resolves_literal() {
        let cfg = Config::load(&args(&["--http-api-token", "mytoken"])).unwrap();
        assert_eq!(cfg.http_api.token, Some("mytoken".to_string()));
    }

    #[test]
    fn notification_template_resolves_literal() {
        let cfg = Config::load(&args(&["--notification-template", "Updated: {{name}}"])).unwrap();
        assert_eq!(
            cfg.notifications.general.template,
            Some("Updated: {{name}}".to_string())
        );
    }

    #[test]
    fn webhook_headers_resolves_literal() {
        let cfg = Config::load(&args(&[
            "--webhook-url",
            "https://example.com/hook",
            "--webhook-headers",
            "X-Token:abc123",
        ]))
        .unwrap();
        let wh = cfg.notifications.webhook.unwrap();
        assert_eq!(wh.headers, Some("X-Token:abc123".to_string()));
    }

    #[test]
    fn email_with_auth_credentials() {
        let cfg = Config::load(&args(&[
            "--notification-email-from",
            "from@example.com",
            "--notification-email-to",
            "to@example.com",
            "--notification-email-server",
            "smtp.example.com",
            "--notification-email-user",
            "user@example.com",
            "--notification-email-password",
            "s3cr3t",
        ]))
        .unwrap();
        let email = cfg.notifications.email.unwrap();
        assert_eq!(email.user, Some("user@example.com".to_string()));
        assert_eq!(email.password, Some("s3cr3t".to_string()));
    }

    #[test]
    fn generate_sample_config_is_valid_toml() {
        let output = generate_sample_config();
        config::Config::builder()
            .add_source(config::File::from_str(&output, config::FileFormat::Toml))
            .build()
            .expect("generate_sample_config must produce valid TOML");
    }

    #[test]
    fn generate_sample_config_loads_with_defaults() {
        let output = generate_sample_config();
        let path = std::env::temp_dir().join("saurron_sample_config_test.toml");
        std::fs::write(&path, &output).unwrap();
        let cfg = Config::load(&args(&["--config", path.to_str().unwrap()])).unwrap();
        std::fs::remove_file(&path).ok();

        assert_eq!(cfg.log_level, LogLevel::Info);
        assert_eq!(cfg.log_format, LogFormat::Auto);
        assert_eq!(cfg.stop_timeout, "10s");
        assert_eq!(cfg.http_api.port, 8080);
        assert!(cfg.rollback.on_exit_code);
        assert!(cfg.rollback.on_healthcheck);
        assert!(cfg.rollback.on_timeout);
        assert_eq!(cfg.rollback.startup_timeout, "30s");
        assert!(cfg.notifications.webhook.is_none());
        assert!(cfg.notifications.email.is_none());
        assert!(cfg.notifications.mqtt.is_none());
        assert!(cfg.notifications.pushover.is_none());
    }

    #[test]
    fn generate_sample_config_contains_all_sections() {
        let output = generate_sample_config();
        for section in &[
            "[docker]",
            "[rollback]",
            "[http_api]",
            "[notifications]",
            "[notifications.webhook]",
            "[notifications.email]",
            "[notifications.mqtt]",
            "[notifications.pushover]",
        ] {
            assert!(
                output.contains(section),
                "missing section {section} in generated config"
            );
        }
    }

    #[test]
    fn mqtt_with_optional_fields() {
        let cfg = Config::load(&args(&[
            "--notification-mqtt-broker",
            "tcp://broker.example.com:1883",
            "--notification-mqtt-topic",
            "saurron/updates",
            "--notification-mqtt-client-id",
            "client-1",
            "--notification-mqtt-username",
            "mqttuser",
            "--notification-mqtt-password",
            "mqttpass",
        ]))
        .unwrap();
        let mqtt = cfg.notifications.mqtt.unwrap();
        assert_eq!(mqtt.client_id, Some("client-1".to_string()));
        assert_eq!(mqtt.username, Some("mqttuser".to_string()));
        assert_eq!(mqtt.password, Some("mqttpass".to_string()));
    }
}
