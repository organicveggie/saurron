---
layout: page
title: Quickstart
nav_order: 1
---

# Table of contents
{: .no_toc }

* TOC
{:toc}

# Docker run

```shell
# Pull from GHCR
docker pull ghcr.io/organicveggie/saurron:latest

# Run — mount the Docker socket and set the polling interval
docker run -d --name saurron --restart=always \
    -v /var/run/docker.sock:/var/run/docker.sock \
    -e SAURRON_POLL_INTERVAL=24h \
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

networks:
  default:
    name: saurron_network
```

Once you have created or downloaded the compose file, you can deploy it with the following command:

```shell
docker compose -f saurron-compose.yaml up -d
```