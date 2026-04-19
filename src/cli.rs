use clap::Parser;
use serde::Deserialize;

#[derive(Parser, Debug)]
#[command(
    name = "saurron",
    version = env!("SAURRON_VERSION"),
    about = "Ever-watchful eye: automatic Docker container updater",
    long_about = None,
)]
pub struct Args {
    // === General ===
    /// Path to TOML config file (default: /etc/saurron/config.toml)
    #[arg(long, env = "SAURRON_CONFIG", value_name = "PATH")]
    pub config: Option<String>,

    /// Log level
    #[arg(long, env = "SAURRON_LOG_LEVEL", value_name = "LEVEL", value_enum)]
    pub log_level: Option<LogLevel>,

    /// Log format
    #[arg(long, env = "SAURRON_LOG_FORMAT", value_name = "FORMAT", value_enum)]
    pub log_format: Option<LogFormat>,

    /// Shorthand for --log-level debug
    #[arg(long, conflicts_with = "trace")]
    pub debug: bool,

    /// Shorthand for --log-level trace
    #[arg(long, conflicts_with = "debug")]
    pub trace: bool,

    /// Path to append-only audit log file
    #[arg(long, env = "SAURRON_AUDIT_LOG", value_name = "PATH")]
    pub audit_log: Option<String>,

    // === Docker Connection ===
    /// Docker daemon socket or host URL (default: unix:///var/run/docker.sock)
    #[arg(long, env = "DOCKER_HOST", value_name = "URI")]
    pub host: Option<String>,

    /// Enable TLS for Docker daemon connection
    #[arg(long, env = "DOCKER_TLS_VERIFY", default_missing_value = "true", num_args = 0..=1)]
    pub tlsverify: Option<bool>,

    /// Path to TLS CA certificate
    #[arg(long, env = "DOCKER_CERT_PATH", value_name = "PATH")]
    pub tls_ca_cert: Option<String>,

    /// Path to TLS client certificate
    #[arg(long, value_name = "PATH")]
    pub tls_cert: Option<String>,

    /// Path to TLS client key
    #[arg(long, value_name = "PATH")]
    pub tls_key: Option<String>,

    /// Docker API version to negotiate (default: auto-negotiate)
    #[arg(long, env = "DOCKER_API_VERSION", value_name = "VERSION")]
    pub api_version: Option<String>,

    // === Scheduling ===
    /// Poll interval as duration (e.g. 5m, 1h); default: 24h
    #[arg(
        long,
        env = "SAURRON_POLL_INTERVAL",
        value_name = "DURATION",
        conflicts_with_all = ["schedule", "run_once"]
    )]
    pub interval: Option<String>,

    /// Poll schedule as cron expression (e.g. "0 4 * * *")
    #[arg(
        long,
        env = "SAURRON_SCHEDULE",
        value_name = "CRON",
        conflicts_with_all = ["interval", "run_once"]
    )]
    pub schedule: Option<String>,

    /// Run a single update cycle then exit
    #[arg(
        long,
        env = "SAURRON_RUN_ONCE",
        default_missing_value = "true",
        num_args = 0..=1,
        conflicts_with_all = ["interval", "schedule"]
    )]
    pub run_once: Option<bool>,

    // === Container Selection ===
    /// Only update containers with saurron.enable=true label
    #[arg(long, env = "SAURRON_LABEL_ENABLE", default_missing_value = "true", num_args = 0..=1)]
    pub label_enable: Option<bool>,

    /// Comma-separated container names to always exclude
    #[arg(
        long,
        env = "SAURRON_DISABLE_CONTAINERS",
        value_name = "NAMES",
        value_delimiter = ','
    )]
    pub disable_containers: Option<Vec<String>>,

    /// Comma-separated container names to consider; all others are ignored
    #[arg(
        long,
        env = "SAURRON_CONTAINERS",
        value_name = "NAMES",
        value_delimiter = ','
    )]
    pub containers: Option<Vec<String>>,

    /// Include containers in restarting state
    #[arg(
        long,
        env = "SAURRON_INCLUDE_RESTARTING",
        default_missing_value = "true",
        num_args = 0..=1
    )]
    pub include_restarting: Option<bool>,

    /// Global flags take precedence over per-container labels for monitor-only and no-pull
    #[arg(
        long,
        env = "SAURRON_GLOBAL_TAKES_PRECEDENCE",
        default_missing_value = "true",
        num_args = 0..=1
    )]
    pub global_takes_precedence: Option<bool>,

    // === Update Strategy ===
    /// Detect and notify only; do not pull or restart
    #[arg(
        long,
        env = "SAURRON_MONITOR_ONLY",
        default_missing_value = "true",
        num_args = 0..=1
    )]
    pub monitor_only: Option<bool>,

    /// Restart using cached image without pulling
    #[arg(
        long,
        env = "SAURRON_NO_PULL",
        default_missing_value = "true",
        num_args = 0..=1
    )]
    pub no_pull: Option<bool>,

    /// Remove old images after successful update
    #[arg(
        long,
        env = "SAURRON_CLEANUP",
        default_missing_value = "true",
        num_args = 0..=1
    )]
    pub cleanup: Option<bool>,

    /// Start stopped containers after image update
    #[arg(
        long,
        env = "SAURRON_REVIVE_STOPPED",
        default_missing_value = "true",
        num_args = 0..=1
    )]
    pub revive_stopped: Option<bool>,

    /// Wait time for graceful stop before SIGKILL (e.g. 10s)
    #[arg(long, env = "SAURRON_STOP_TIMEOUT", value_name = "DURATION")]
    pub stop_timeout: Option<String>,

    // === Rollback ===
    /// Rollback if new container exits non-zero (default: enabled)
    #[arg(
        long,
        env = "SAURRON_ROLLBACK_ON_EXIT_CODE",
        overrides_with = "no_rollback_on_exit_code",
        default_missing_value = "true",
        num_args = 0..=1
    )]
    pub rollback_on_exit_code: Option<bool>,

    /// Disable rollback on non-zero exit
    #[arg(long, overrides_with = "rollback_on_exit_code", hide = true)]
    pub no_rollback_on_exit_code: bool,

    /// Rollback if Docker healthcheck reports unhealthy (default: enabled)
    #[arg(
        long,
        env = "SAURRON_ROLLBACK_ON_HEALTHCHECK",
        overrides_with = "no_rollback_on_healthcheck",
        default_missing_value = "true",
        num_args = 0..=1
    )]
    pub rollback_on_healthcheck: Option<bool>,

    /// Disable rollback on healthcheck failure
    #[arg(long, overrides_with = "rollback_on_healthcheck", hide = true)]
    pub no_rollback_on_healthcheck: bool,

    /// Rollback if container doesn't reach running within startup timeout (default: enabled)
    #[arg(
        long,
        env = "SAURRON_ROLLBACK_ON_TIMEOUT",
        overrides_with = "no_rollback_on_timeout",
        default_missing_value = "true",
        num_args = 0..=1
    )]
    pub rollback_on_timeout: Option<bool>,

    /// Disable rollback on startup timeout
    #[arg(long, overrides_with = "rollback_on_timeout", hide = true)]
    pub no_rollback_on_timeout: bool,

    /// Wait time before triggering rollback (e.g. 30s)
    #[arg(long, env = "SAURRON_STARTUP_TIMEOUT", value_name = "DURATION")]
    pub startup_timeout: Option<String>,

    // === Registry ===
    /// Warning behaviour for failed HEAD requests
    #[arg(
        long,
        env = "SAURRON_HEAD_WARN_STRATEGY",
        value_name = "STRATEGY",
        value_enum
    )]
    pub head_warn_strategy: Option<HeadWarnStrategy>,

    // === HTTP API ===
    /// Enable POST /v1/update endpoint
    #[arg(
        long,
        env = "SAURRON_HTTP_API_UPDATE",
        default_missing_value = "true",
        num_args = 0..=1
    )]
    pub http_api_update: Option<bool>,

    /// Enable GET /v1/metrics endpoint
    #[arg(
        long,
        env = "SAURRON_HTTP_API_METRICS",
        default_missing_value = "true",
        num_args = 0..=1
    )]
    pub http_api_metrics: Option<bool>,

    /// Bearer token for all API requests
    #[arg(long, env = "SAURRON_HTTP_API_TOKEN", value_name = "TOKEN")]
    pub http_api_token: Option<String>,

    /// HTTP API server port (default: 8080)
    #[arg(long, env = "SAURRON_HTTP_API_PORT", value_name = "PORT")]
    pub http_api_port: Option<u16>,

    /// Serve GET /v1/metrics without Bearer token
    #[arg(
        long,
        env = "SAURRON_HTTP_API_METRICS_NO_AUTH",
        default_missing_value = "true",
        num_args = 0..=1
    )]
    pub http_api_metrics_no_auth: Option<bool>,

    // === Notifications — General ===
    /// Delay between cycle completion and notification dispatch (e.g. 30s)
    #[arg(long, env = "SAURRON_NOTIFICATION_DELAY", value_name = "DURATION")]
    pub notification_delay: Option<String>,

    /// Custom notification template string
    #[arg(long, env = "SAURRON_NOTIFICATION_TEMPLATE", value_name = "TEMPLATE")]
    pub notification_template: Option<String>,

    // === Notifications — Webhook ===
    /// URL to POST notification payloads to
    #[arg(long, env = "SAURRON_WEBHOOK_URL", value_name = "URL")]
    pub webhook_url: Option<String>,

    /// Additional HTTP headers as comma-separated Key:Value pairs
    #[arg(long, env = "SAURRON_WEBHOOK_HEADERS", value_name = "HEADERS")]
    pub webhook_headers: Option<String>,

    /// Skip TLS cert verification for webhook
    #[arg(
        long,
        env = "SAURRON_WEBHOOK_TLS_SKIP_VERIFY",
        default_missing_value = "true",
        num_args = 0..=1
    )]
    pub webhook_tls_skip_verify: Option<bool>,

    // === Notifications — Email ===
    /// Sender email address
    #[arg(long, env = "SAURRON_NOTIFICATION_EMAIL_FROM", value_name = "ADDRESS")]
    pub notification_email_from: Option<String>,

    /// Recipient email address(es), comma-separated
    #[arg(
        long,
        env = "SAURRON_NOTIFICATION_EMAIL_TO",
        value_name = "ADDRESSES",
        value_delimiter = ','
    )]
    pub notification_email_to: Option<Vec<String>>,

    /// SMTP server hostname
    #[arg(long, env = "SAURRON_NOTIFICATION_EMAIL_SERVER", value_name = "HOST")]
    pub notification_email_server: Option<String>,

    /// SMTP server port (default: 587)
    #[arg(long, env = "SAURRON_NOTIFICATION_EMAIL_PORT", value_name = "PORT")]
    pub notification_email_port: Option<u16>,

    /// SMTP auth username
    #[arg(long, env = "SAURRON_NOTIFICATION_EMAIL_USER", value_name = "USER")]
    pub notification_email_user: Option<String>,

    /// SMTP auth password
    #[arg(
        long,
        env = "SAURRON_NOTIFICATION_EMAIL_PASSWORD",
        value_name = "PASSWORD"
    )]
    pub notification_email_password: Option<String>,

    /// Skip TLS cert verification for SMTP
    #[arg(
        long,
        env = "SAURRON_NOTIFICATION_EMAIL_TLS_SKIP_VERIFY",
        default_missing_value = "true",
        num_args = 0..=1
    )]
    pub notification_email_tls_skip_verify: Option<bool>,

    // === Notifications — MQTT ===
    /// MQTT broker URL (e.g. tcp://broker.example.com:1883)
    #[arg(long, env = "SAURRON_NOTIFICATION_MQTT_BROKER", value_name = "URL")]
    pub notification_mqtt_broker: Option<String>,

    /// MQTT topic for notifications
    #[arg(long, env = "SAURRON_NOTIFICATION_MQTT_TOPIC", value_name = "TOPIC")]
    pub notification_mqtt_topic: Option<String>,

    /// MQTT QoS level: 0 (at most once), 1 (at least once), 2 (exactly once)
    #[arg(long, env = "SAURRON_NOTIFICATION_MQTT_QOS", value_name = "LEVEL")]
    pub notification_mqtt_qos: Option<u8>,

    /// MQTT client ID (auto-generated if omitted)
    #[arg(long, env = "SAURRON_NOTIFICATION_MQTT_CLIENT_ID", value_name = "ID")]
    pub notification_mqtt_client_id: Option<String>,

    /// MQTT broker auth username
    #[arg(long, env = "SAURRON_NOTIFICATION_MQTT_USERNAME", value_name = "USER")]
    pub notification_mqtt_username: Option<String>,

    /// MQTT broker auth password
    #[arg(
        long,
        env = "SAURRON_NOTIFICATION_MQTT_PASSWORD",
        value_name = "PASSWORD"
    )]
    pub notification_mqtt_password: Option<String>,

    // === Notifications — Pushover ===
    /// Pushover application API token
    #[arg(
        long,
        env = "SAURRON_NOTIFICATION_PUSHOVER_TOKEN",
        value_name = "TOKEN"
    )]
    pub notification_pushover_token: Option<String>,

    /// Pushover user or group key
    #[arg(
        long,
        env = "SAURRON_NOTIFICATION_PUSHOVER_USER_KEY",
        value_name = "KEY"
    )]
    pub notification_pushover_user_key: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Auto,
    Json,
    Logfmt,
    Pretty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum HeadWarnStrategy {
    Auto,
    Always,
    Never,
}
