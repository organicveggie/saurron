mod cli;
mod config;
mod docker;

use clap::Parser;
use tracing::info;

const VERSION: &str = env!("SAURRON_VERSION");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = cli::Args::parse();

    // Phase 1: minimal tracing init (pretty format, level from CLI/env, default INFO).
    // Full format config (json/logfmt/auto) wired in Phase 3.
    let level = if args.trace {
        tracing::Level::TRACE
    } else if args.debug {
        tracing::Level::DEBUG
    } else {
        match args.log_level {
            Some(cli::LogLevel::Trace) => tracing::Level::TRACE,
            Some(cli::LogLevel::Debug) => tracing::Level::DEBUG,
            Some(cli::LogLevel::Warn) => tracing::Level::WARN,
            Some(cli::LogLevel::Error) => tracing::Level::ERROR,
            _ => tracing::Level::INFO,
        }
    };

    tracing_subscriber::fmt().with_max_level(level).init();

    let config = config::Config::load(&args)?;

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
