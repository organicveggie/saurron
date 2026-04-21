mod audit;
mod cli;
mod config;
mod docker;

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

    Ok(())
}
