---
layout: page
title: Container Registries
---

<!-- prettier-ignore-start -->
# Container Registries
{: .no_toc }

<!-- prettier-ignore-end -->

How Saurron communicates with image registries to detect updates, authenticate, and handle
rate limiting.

<!-- prettier-ignore-start -->
* TOC
{:toc}

<!-- prettier-ignore-end -->

## Supported registries

Saurron works with any registry that implements the
[Docker Registry HTTP API v2](https://distribution.github.io/distribution/spec/api/), which
includes:

- **Docker Hub** (`docker.io` / `registry-1.docker.io`)
- **GitHub Container Registry** (`ghcr.io`)
- **Amazon ECR**, **Google Artifact Registry**, **Azure Container Registry**
- Self-hosted registries (Docker distribution, Harbor, Gitea, Zot, etc.)

`localhost` and bare-IP registries (e.g. `172.17.0.1:5000`) are treated as plain HTTP;
all others use HTTPS.

## How freshness is checked

Saurron does not pull images to check for updates. Instead it uses two lightweight strategies
depending on the image tag:

**Digest comparison (non-semver tags such as `latest`)**
: A `HEAD` request is sent to `GET /v2/{repository}/manifests/{tag}`. The
`Docker-Content-Digest` header from the response is compared against the digest of the
currently running container. A mismatch means the image is stale.

**SemVer tag listing**
: When the running tag parses as a semantic version (e.g. `1.25.3`), Saurron fetches
`GET /v2/{repository}/tags/list` and selects the highest available tag that is strictly
greater than the current version. No digest comparison is needed.

## Authentication

Registries that require authentication return a `401 Unauthorized` on the initial request.
Saurron handles this automatically: it extracts the `WWW-Authenticate: Bearer` challenge,
fetches a scoped token from the token endpoint (using Basic Auth if credentials are
configured), and retries the original request with the token attached.

**Docker Hub** uses a separate OAuth token endpoint (`hub.docker.com/v2/auth/token`) when
credentials are provided. Anonymous access still works for public images via the standard
`auth.docker.io/token` flow.

Credentials are configured in two ways:

- **Global** — `--registry-username` / `--registry-password` apply to any registry that
  has no explicit per-registry entry.
- **Per-registry** — `[[registry_credentials]]` TOML entries let you pin credentials (or
  force anonymous access) for individual registries, taking priority over global credentials.

See [Registry configuration](config/registry.md) for full details and examples.

## Docker Hub rate limiting

Docker Hub enforces per-IP and per-authenticated-user pull rate limits. When the API returns
a `429 Too Many Requests` response, Saurron inspects the `Retry-After` header and decides
whether to wait and retry:

| `Retry-After` value | Behaviour |
| ------------------- | --------- |
| ≤ 10 seconds | Log a warning, sleep for the specified duration, and retry the request. Up to 3 retries are attempted. |
| > 10 seconds | Log a warning including the requested wait time and return an error for that image. The rest of the cycle continues normally. |
| Header absent | Log a warning and return an error immediately. |

When a request does succeed, Saurron checks the `X-RateLimit-Remaining` response header. If
the remaining quota drops to 5 or fewer requests, a warning is emitted so the situation is
visible in logs before the limit is fully exhausted.

Rate limit errors surface as `FreshnessResult::Error` for the affected image, which is
recorded in the session report. Other containers in the same cycle are unaffected.

Rate limiting is applied only to Docker Hub (`registry-1.docker.io`). Other registries that
return `429` are treated as unexpected errors without retry logic.

## HEAD warn strategy

Some registries do not support `HEAD` requests on manifests and return errors that do not
indicate a genuine problem. `--head-warn-strategy` controls how these failures are surfaced
in logs:

| Value | Behaviour |
| ----- | --------- |
| `auto` (default) | Warn only for Docker Hub and `ghcr.io`; suppress for all others. |
| `always` | Warn on every HEAD failure regardless of registry. |
| `never` | Suppress all HEAD failure warnings. |

See [Registry configuration](config/registry.md#head-warn-strategy) for details.
