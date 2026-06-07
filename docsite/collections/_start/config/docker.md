---
layout: page
title: Docker
parent: Configuration
---

<!-- prettier-ignore-start -->
# Docker Connection
{: .no_toc }

<!-- prettier-ignore-end -->

Settings to control how Saurron connects to the Docker daemon. By default Saurron connects via the
Unix socket at `/var/run/docker.sock`, which must be mounted into the Saurron container. TLS
connections to a remote daemon are also supported.

<!-- prettier-ignore-start -->
* TOC
{:toc}

<!-- prettier-ignore-end -->

## Host

CLI flag
: `--host <uri>`

Environment
: `DOCKER_HOST`

TOML key
: `docker.host`

Docker daemon socket or host URL. Default: `unix:///var/run/docker.sock`.

Use a `tcp://` or `https://` URI to connect to a remote daemon. When connecting over TCP with TLS,
also set `--tlsverify` and the associated certificate options.

## API version

CLI flag
: `--api-version <version>`

Environment
: `DOCKER_API_VERSION`

TOML key
: `docker.api_version`

Docker API version to use when communicating with the daemon (e.g. `1.41`). By default Saurron
auto-negotiates the highest version supported by the daemon. Set this explicitly when connecting to
an older daemon that reports an incompatible version during negotiation.

## TLS verify

CLI flag
: `--tlsverify`

Environment
: `DOCKER_TLS_VERIFY`

TOML key
: `docker.tls_verify`

Enable TLS for the Docker daemon connection and verify the server certificate. When set, the CA
certificate (`--tls-ca-cert`), client certificate (`--tls-cert`), and client key (`--tls-key`) must
also be provided.

## TLS CA certificate

CLI flag
: `--tls-ca-cert <path>`

Environment
: `DOCKER_CERT_PATH`

TOML key
: `docker.tls_ca_cert`

Path to the TLS CA certificate used to verify the Docker daemon's server certificate.

## TLS client certificate

CLI flag
: `--tls-cert <path>`

TOML key
: `docker.tls_cert`

Path to the TLS client certificate presented to the Docker daemon for mutual TLS authentication.

## TLS client key

CLI flag
: `--tls-key <path>`

TOML key
: `docker.tls_key`

Path to the TLS client private key corresponding to the client certificate.
