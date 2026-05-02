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

| Field | Impact |
| --- | --- |
| `security_opt` | AppArmor and seccomp profiles are dropped. Containers running with a custom security profile will restart without it. |
| `memory`, `memory_swap`, `memory_reservation`, `nano_cpus`, `cpu_shares`, `cpu_period`, `cpu_quota`, `cpuset_cpus`, `cpuset_mems` | All resource constraints (CPU and memory limits) are lost on update. |
| `tmpfs` | tmpfs mount definitions are not preserved. |
| `dns`, `dns_search`, `dns_options` | Custom DNS server and search-domain settings are dropped. |
| `runtime` | Container runtime override (e.g. `nvidia` for GPU containers) is lost. GPU workloads will fail to start after update. |

### Lower priority

| Field | Impact |
| --- | --- |
| `sysctls` | Kernel parameter overrides (`--sysctl`) are not preserved. |
| `pid_mode` | PID namespace sharing (`--pid`) is dropped. |
| `ipc_mode` | IPC namespace sharing (`--ipc`) is dropped. |
| `userns_mode` | User namespace remapping (`--userns`) is dropped. |
| `readonly_rootfs` | Read-only root filesystem flag (`--read-only`) is not preserved. |
| `pids_limit` | Per-container PID limit is dropped. |
