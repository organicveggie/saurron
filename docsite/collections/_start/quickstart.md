---
layout: page
title: Quickstart
nav_order: 1
---

# Table of contents
{: .no_toc }

* TOC
{:toc}

{: .note }
Details on how to install Docker can be found on the [official Docker website](https://docs.docker.com/get-docker/).

# Docker CLI

```shell
# Pull from GHCR
docker pull ghcr.io/organicveggie/saurron:latest

# Run — mount the Docker socket and set the polling interval
docker run -d --name saurron --restart=always \
    -v /var/run/docker.sock:/var/run/docker.sock \
    -e SAURRON_POLL_INTERVAL=24h \
    -e TZ=America/New_York
    ghcr.io/organicveggie/saurron:latest
```

# Docker compose

Create a `saurron-compose.yml` file with the following contents:

```yaml
services:
  saurron:
    container_name: saurron
    image: organicveggie/saurron:latest
    restart: always
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
    ports:
      - 8080:8080
    environment:
      TZ: Australia/Sydney

networks:
  default:
    name: saurron_network
```

Once you have created or downloaded the compose file, you can deploy it with the following command:

```shell
docker compose -f saurron-compose.yaml up -d
```

# Pre-built binaries

Download a tarball for your platform from the [latest release](https://github.com/organicveggie/saurron/releases/latest):

```bash
# Linux amd64
curl -Lo saurron.tar.gz https://github.com/organicveggie/saurron/releases/latest/download/saurron-latest-linux-amd64.tar.gz
tar -xzf saurron.tar.gz
./saurron --help
```

## Run from binary

```bash
# Connect to local Docker daemon and enumerate containers
./saurron

# Single update cycle, then exit
./saurron --run-once

# Monitor only — detect stale images but do not update
./saurron --monitor-only

# JSON log output
./saurron --log-format json

# Write audit events to a dedicated file
./saurron --audit-log /var/log/saurron/audit.log
```
