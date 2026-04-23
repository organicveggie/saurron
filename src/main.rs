mod audit;
mod cli;
mod config;
mod docker;
mod http;
mod registry;
mod scheduler;
mod selfupdate;
mod update;

use std::sync::Arc;

use anyhow::Context as _;
use clap::Parser;
use tracing::info;

const VERSION: &str = env!("SAURRON_VERSION");

fn init_tracing(
    config: &config::Config,
) -> anyhow::Result<Option<tracing_appender::non_blocking::WorkerGuard>> {
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
        cli::LogFormat::Json => tracing_subscriber::fmt::layer().json().boxed(),
        cli::LogFormat::Pretty => tracing_subscriber::fmt::layer().pretty().boxed(),
        cli::LogFormat::Logfmt => tracing_logfmt::layer().boxed(),
        cli::LogFormat::Auto => unreachable!(),
    };

    let mut guard: Option<tracing_appender::non_blocking::WorkerGuard> = None;
    let audit_layer: Option<BoxLayer> = if let Some(ref path) = config.audit_log {
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
        guard = Some(g);
        let layer = tracing_subscriber::fmt::layer()
            .json()
            .with_writer(non_blocking)
            .with_filter(tracing_subscriber::filter::filter_fn(|meta| {
                meta.target() == "saurron::audit"
            }))
            .boxed();
        Some(layer)
    } else {
        None
    };

    let mut layers: Vec<BoxLayer> = vec![stdout_layer];
    if let Some(layer) = audit_layer {
        layers.push(layer);
    }

    tracing_subscriber::registry()
        .with(layers)
        .with(tracing_subscriber::EnvFilter::from_default_env().add_directive(level.into()))
        .init();

    Ok(guard)
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
    let config = config::Config::load(&args)?;
    let _guard = init_tracing(&config)?;

    info!(
        version = VERSION,
        docker_host = %config.docker.host,
        run_once = config.run_once,
        monitor_only = config.monitor_only,
        "Saurron starting"
    );

    // Validate HTTP API token config before binding any ports.
    http::validate_token_config(&config.http_api)?;

    // Validate scheduling flags (clap catches CLI conflicts; this catches TOML combinations).
    let schedule_mode = scheduler::parse_schedule_mode(&config)?;

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
