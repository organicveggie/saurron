# Saurron — Known Bugs and Missing Features

## Container recreation: missing run-config fields

When Saurron updates a container it stops the old one, removes it, and recreates it from the
new image using a captured snapshot of the original run config (`extract_run_config` /
`build_create_config` in `src/update.rs`). The following fields are **not captured**, so they
are silently dropped on every update.

### High priority

All four high-priority items were fixed in `src/update.rs` (`ContainerRunConfig`,
`extract_run_config`, `build_create_config`). Each field is now captured from the old
container's inspect response and applied when recreating the container.

| Field | Status |
| --- | --- |
| `group_add` | **Fixed** — supplementary groups are now preserved across updates. |
| `healthcheck` | **Fixed** — run-time healthcheck config is now captured and re-applied. |
| `mounts` (`host_config`) | **Fixed** — `--mount` syntax mounts are now preserved alongside `binds`. |
| `volumes` (`config`) | **Fixed** — anonymous volume declarations are now captured and re-applied. |

### Medium priority

All five medium-priority items were fixed in `src/update.rs` (`ContainerRunConfig`,
`extract_run_config`, `build_create_config`).

| Field | Status |
| --- | --- |
| `security_opt` | **Fixed** — AppArmor and seccomp profiles are now preserved across updates. |
| `memory`, `memory_swap`, `memory_reservation`, `nano_cpus`, `cpu_shares`, `cpu_period`, `cpu_quota`, `cpuset_cpus`, `cpuset_mems` | **Fixed** — all CPU and memory resource constraints are now preserved. |
| `tmpfs` | **Fixed** — tmpfs mount definitions are now preserved. |
| `dns`, `dns_search`, `dns_options` | **Fixed** — custom DNS settings are now preserved. |
| `runtime` | **Fixed** — container runtime override (e.g. `nvidia`) is now preserved. |

### Lower priority

All six lower-priority items were fixed in `src/update.rs` (`ContainerRunConfig`,
`extract_run_config`, `build_create_config`).

| Field | Status |
| --- | --- |
| `sysctls` | **Fixed** — kernel parameter overrides are now preserved. |
| `pid_mode` | **Fixed** — PID namespace sharing is now preserved. |
| `ipc_mode` | **Fixed** — IPC namespace sharing is now preserved. |
| `userns_mode` | **Fixed** — user namespace remapping is now preserved. |
| `readonly_rootfs` | **Fixed** — read-only root filesystem flag is now preserved. |
| `pids_limit` | **Fixed** — per-container PID limit is now preserved. |
