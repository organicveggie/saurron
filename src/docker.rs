use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use bollard::API_DEFAULT_VERSION;
use bollard::Docker;

use crate::config::DockerConfig;

pub struct DockerClient {
    inner: Docker,
}

#[derive(Debug, PartialEq)]
enum ConnectionType {
    Socket,
    Http,
    Https,
}

fn connection_type(host: &str, tls_verify: bool) -> ConnectionType {
    if host.starts_with("unix://") || host.starts_with("npipe://") || host.starts_with('/') {
        ConnectionType::Socket
    } else if tls_verify || host.starts_with("https://") {
        ConnectionType::Https
    } else {
        ConnectionType::Http
    }
}

fn parse_api_version(version: &str) -> Result<bollard::ClientVersion> {
    let v = version.trim_start_matches('v');
    let (major_str, minor_str) = v
        .split_once('.')
        .context("API version must be in 'major.minor' format")?;
    let major_version = major_str
        .parse::<usize>()
        .context("invalid API version major component")?;
    let minor_version = minor_str
        .parse::<usize>()
        .context("invalid API version minor component")?;
    Ok(bollard::ClientVersion {
        major_version,
        minor_version,
    })
}

impl DockerClient {
    pub fn connect(config: &DockerConfig) -> Result<Self> {
        let api_version = config
            .api_version
            .as_deref()
            .map(parse_api_version)
            .transpose()?;
        let version = api_version.as_ref().unwrap_or(API_DEFAULT_VERSION);

        let inner = match connection_type(&config.host, config.tls_verify) {
            ConnectionType::Socket => Docker::connect_with_socket(&config.host, 120, version)
                .context("failed to connect to Docker via Unix socket")?,

            ConnectionType::Http => Docker::connect_with_http(&config.host, 120, version)
                .context("failed to connect to Docker via HTTP")?,

            ConnectionType::Https => {
                let ca = config
                    .tls_ca_cert
                    .as_deref()
                    .context("--tls-ca-cert is required when --tlsverify is set")?;

                let (key_path, cert_path): (PathBuf, PathBuf) =
                    match (&config.tls_key, &config.tls_cert) {
                        (Some(k), Some(c)) => (PathBuf::from(k), PathBuf::from(c)),
                        (None, None) => {
                            // Server-only TLS: bollard resolver returns None → no client auth
                            (PathBuf::new(), PathBuf::new())
                        }
                        _ => bail!("--tls-cert and --tls-key must be provided together"),
                    };

                Docker::connect_with_ssl(
                    &config.host,
                    &key_path,
                    &cert_path,
                    Path::new(ca),
                    120,
                    version,
                )
                .context("failed to connect to Docker via TLS")?
            }
        };

        Ok(Self { inner })
    }

    pub async fn ping(&self) -> Result<()> {
        self.inner
            .ping()
            .await
            .context("Docker daemon ping failed")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_socket_by_scheme() {
        assert_eq!(
            connection_type("unix:///var/run/docker.sock", false),
            ConnectionType::Socket
        );
    }

    #[test]
    fn unix_socket_by_path() {
        assert_eq!(
            connection_type("/var/run/docker.sock", false),
            ConnectionType::Socket
        );
    }

    #[test]
    fn npipe_is_socket() {
        assert_eq!(
            connection_type("npipe:////./pipe/docker_engine", false),
            ConnectionType::Socket
        );
    }

    #[test]
    fn tcp_without_tls_is_http() {
        assert_eq!(
            connection_type("tcp://localhost:2375", false),
            ConnectionType::Http
        );
    }

    #[test]
    fn http_scheme_is_http() {
        assert_eq!(
            connection_type("http://localhost:2375", false),
            ConnectionType::Http
        );
    }

    #[test]
    fn tls_verify_flag_forces_https() {
        assert_eq!(
            connection_type("tcp://localhost:2376", true),
            ConnectionType::Https
        );
    }

    #[test]
    fn https_scheme_selects_https() {
        assert_eq!(
            connection_type("https://localhost:2376", false),
            ConnectionType::Https
        );
    }

    #[test]
    fn parse_api_version_standard() {
        let v = parse_api_version("1.44").unwrap();
        assert_eq!(v.major_version, 1);
        assert_eq!(v.minor_version, 44);
    }

    #[test]
    fn parse_api_version_with_v_prefix() {
        let v = parse_api_version("v1.44").unwrap();
        assert_eq!(v.major_version, 1);
        assert_eq!(v.minor_version, 44);
    }

    #[test]
    fn parse_api_version_invalid_format() {
        assert!(parse_api_version("not-a-version").is_err());
    }

    #[test]
    fn parse_api_version_missing_minor() {
        assert!(parse_api_version("1").is_err());
    }
}
