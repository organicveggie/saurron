use saurron::{cli, config, docker, http, registry, scheduler};
use std::sync::Arc;

use anyhow::Context as _;
use clap::Parser;
use tracing::info;

const VERSION: &str = env!("SAURRON_VERSION");

/// `tracing-subscriber` defaults to UTC timestamps regardless of the `TZ` environment variable.
/// This timer uses `chrono::Local` so the configured timezone is reflected in all log output.
struct LocalTime;

impl tracing_subscriber::fmt::time::FormatTime for LocalTime {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        write!(
            w,
            "{}",
            chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, false)
        )
    }
}

fn init_tracing(
    config: &config::Config,
) -> anyhow::Result<Vec<tracing_appender::non_blocking::WorkerGuard>> {
    use std::io::IsTerminal;
    use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};

    let level = match config.log_level {
        cli::LogLevel::Trace => tracing::Level::TRACE,
        cli::LogLevel::Debug => tracing::Level::DEBUG,
        cli::LogLevel::Info => tracing::Level::INFO,
        cli::LogLevel::Warn => tracing::Level::WARN,
        cli::LogLevel::Error => tracing::Level::ERROR,
    };

    let effective_format = match config.log_format {
        cli::LogFormat::Auto => {
            if std::io::stdout().is_terminal() {
                cli::LogFormat::Pretty
            } else {
                cli::LogFormat::Logfmt
            }
        }
        f => f,
    };

    type BoxLayer = Box<dyn Layer<tracing_subscriber::Registry> + Send + Sync>;

    let stdout_layer: BoxLayer = match effective_format {
        cli::LogFormat::Json => tracing_subscriber::fmt::layer()
            .with_timer(LocalTime)
            .json()
            .boxed(),
        cli::LogFormat::Pretty => tracing_subscriber::fmt::layer()
            .with_timer(LocalTime)
            .pretty()
            .boxed(),
        // tracing_logfmt hardcodes UTC and does not support a custom timer.
        cli::LogFormat::Logfmt => tracing_logfmt::layer().boxed(),
        cli::LogFormat::Auto => unreachable!(),
    };

    let mut guards: Vec<tracing_appender::non_blocking::WorkerGuard> = Vec::new();
    let mut layers: Vec<BoxLayer> = vec![stdout_layer];

    if let Some(ref path) = config.audit_log {
        let p = std::path::Path::new(path);
        let dir = p.parent().unwrap_or_else(|| std::path::Path::new("."));
        let filename = p
            .file_name()
            .context("audit_log path must include a filename")?
            .to_string_lossy()
            .into_owned();
        std::fs::create_dir_all(dir)
            .with_context(|| format!("failed to create audit log directory: {}", dir.display()))?;
        let appender = tracing_appender::rolling::never(dir, &filename);
        let (non_blocking, g) = tracing_appender::non_blocking(appender);
        guards.push(g);
        layers.push(
            tracing_subscriber::fmt::layer()
                .with_timer(LocalTime)
                .json()
                .with_writer(non_blocking)
                .with_filter(tracing_subscriber::filter::filter_fn(|meta| {
                    meta.target() == "saurron::audit"
                }))
                .boxed(),
        );
    }

    if let Some(ref path) = config.http_api.access_log {
        let p = std::path::Path::new(path);
        let dir = p.parent().unwrap_or_else(|| std::path::Path::new("."));
        let filename = p
            .file_name()
            .context("http_api.access_log path must include a filename")?
            .to_string_lossy()
            .into_owned();
        std::fs::create_dir_all(dir)
            .with_context(|| format!("failed to create access log directory: {}", dir.display()))?;
        let appender = tracing_appender::rolling::never(dir, &filename);
        let (non_blocking, g) = tracing_appender::non_blocking(appender);
        guards.push(g);
        layers.push(
            tracing_subscriber::fmt::layer()
                .with_timer(LocalTime)
                .json()
                .with_writer(non_blocking)
                .with_filter(tracing_subscriber::filter::filter_fn(|meta| {
                    meta.target() == "saurron::access"
                }))
                .boxed(),
        );
    }

    tracing_subscriber::registry()
        .with(layers)
        .with(tracing_subscriber::EnvFilter::from_default_env().add_directive(level.into()))
        .init();

    Ok(guards)
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut sigterm =
            signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
        tokio::select! {
            _ = sigterm.recv() => { tracing::info!("SIGTERM received"); }
            _ = tokio::signal::ctrl_c() => { tracing::info!("SIGINT received"); }
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await.ok();
        tracing::info!("shutdown signal received");
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = cli::Args::parse();

    // Handle --generate-config before loading config or connecting to Docker,
    // so it works even when no config file or Docker daemon is present.
    if let Some(dest) = &args.generate_config {
        let content = config::generate_sample_config();
        if dest == "-" {
            print!("{content}");
        } else {
            std::fs::write(dest, &content)
                .with_context(|| format!("failed to write config to '{dest}'"))?;
        }
        return Ok(());
    }

    let (config, config_status) = config::Config::load(&args)?;
    let _guards = init_tracing(&config)?;

    info!(version = VERSION, "Saurron starting");
    config_status.log();
    if config_status.is_error() {
        return Err(anyhow::anyhow!("failed to load config file"));
    }
    config.log_settings();

    if config.http_api.access_log.is_some() && !config.http_api.update && !config.http_api.metrics {
        tracing::warn!(
            "http_api.access_log is configured but the HTTP API is not enabled; \
             access log will not be written"
        );
    }

    // Validate HTTP API token config before binding any ports.
    http::validate_token_config(&config.http_api)?;

    // Validate scheduling flags (clap catches CLI conflicts; this catches TOML combinations).
    let schedule_mode = scheduler::parse_schedule_mode(&config)?;

    match &schedule_mode {
        scheduler::ScheduleMode::RunOnce => {
            info!(mode = "run-once", "schedule configured");
        }
        scheduler::ScheduleMode::Interval(_) => {
            let interval = config.poll_interval.as_deref().unwrap_or("24h");
            info!(
                mode = "interval",
                interval,
                first_run = "immediate",
                "schedule configured"
            );
        }
        scheduler::ScheduleMode::Cron(_) => {
            let expression = config.schedule.as_deref().unwrap_or("");
            if let Some(next) = schedule_mode.next_run() {
                info!(mode = "cron", expression, next_run = %next, "schedule configured");
            }
        }
    }

    let docker = docker::DockerClient::connect(&config.docker)?;
    docker.ping().await?;
    info!("Connected to Docker daemon");

    let selector = docker::ContainerSelector::new(
        config.label_enable,
        config.global_takes_precedence,
        &config.disable_containers,
        &config.containers,
        config.include_restarting,
        config.revive_stopped,
    );

    // Initial enumeration for startup logging only.
    let all_containers = docker.list_containers(&selector).await?;
    let selected = docker.select_containers(&all_containers, &selector);
    info!(
        total = all_containers.len(),
        selected = selected.len(),
        "Container enumeration complete"
    );
    for c in &selected {
        info!(id = %c.id, name = %c.name, image = %c.image, state = %c.state, "Container selected");
    }

    let credentials = match (
        config.registry_username.clone(),
        config.registry_password.clone(),
    ) {
        (Some(u), Some(p)) => Some((u, p)),
        _ => None,
    };
    let registry_client =
        registry::RegistryClient::new(config.head_warn_strategy, VERSION, credentials)
            .context("failed to initialise registry client")?;

    let state = Arc::new(http::AppStateInner {
        docker,
        registry: registry_client,
        config,
        selector,
        update_lock: tokio::sync::Mutex::new(()),
    });

    let http_enabled = state.config.http_api.update || state.config.http_api.metrics;

    if matches!(schedule_mode, scheduler::ScheduleMode::RunOnce) {
        http::run_cycle_with_state(&state).await;
        return Ok(());
    }

    let state_for_scheduler = Arc::clone(&state);
    let scheduler_task = tokio::spawn(async move {
        scheduler::run_scheduler(schedule_mode, move || {
            let s = Arc::clone(&state_for_scheduler);
            async move {
                let _guard = s.update_lock.lock().await;
                http::run_cycle_with_state(&s).await;
            }
        })
        .await;
    });

    if http_enabled {
        tokio::select! {
            result = http::start_server(Arc::clone(&state)) => { result?; }
            _ = scheduler_task => {}
            _ = shutdown_signal() => {
                info!("Shutdown signal received; waiting for active update cycle to complete");
                let _ = state.update_lock.lock().await;
                info!("Graceful shutdown complete");
            }
        }
    } else {
        tokio::select! {
            _ = scheduler_task => {}
            _ = shutdown_signal() => {
                info!("Shutdown signal received; waiting for active update cycle to complete");
                let _ = state.update_lock.lock().await;
                info!("Graceful shutdown complete");
            }
        }
    }

    Ok(())
}
