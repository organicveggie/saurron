/// Returns true when `s` looks like a Docker-generated container ID:
/// exactly 12 or 64 lowercase hex characters. Docker sets $HOSTNAME to
/// the first 12 chars of the container ID; /etc/hostname contains the same.
fn looks_like_container_id(s: &str) -> bool {
    (s.len() == 12 || s.len() == 64) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Extract a 64-char hex container ID from one line of /proc/self/cgroup.
///
/// cgroupv1 example: `12:devices:/docker/<64-char-id>`
/// cgroupv2 example: `0::/system.slice/docker-<64-char-id>.scope`
fn extract_id_from_cgroup_line(line: &str) -> Option<String> {
    let path = line.splitn(3, ':').nth(2)?;
    for segment in path.split(['/', '-', '.']) {
        if segment.len() == 64 && segment.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Some(segment.to_string());
        }
    }
    None
}

fn detect_from_cgroup(cgroup_path: &str) -> Option<String> {
    let content = std::fs::read_to_string(cgroup_path).ok()?;
    content.lines().find_map(extract_id_from_cgroup_line)
}

/// Detect Saurron's own Docker container ID.
///
/// Docker sets `$HOSTNAME` to the first 12 characters of the container ID.
/// Falls back to `/proc/self/cgroup` (reliable even when `hostname:` overrides
/// `$HOSTNAME`), then to `/etc/hostname`. Returns `None` when running outside
/// a container or when detection fails.
pub(crate) fn detect_own_container_id() -> Option<String> {
    let hostname_env = std::env::var("HOSTNAME").ok();
    detect_own_container_id_inner(
        hostname_env.as_deref(),
        "/etc/hostname",
        "/proc/self/cgroup",
    )
}

fn detect_own_container_id_inner(
    hostname_value: Option<&str>,
    hostname_path: &str,
    cgroup_path: &str,
) -> Option<String> {
    // $HOSTNAME is reliable only when Docker generated it (12 hex chars).
    // A `hostname:` directive in docker-compose replaces it with a human-readable
    // name, which would cause false-negative self-detection.
    if let Some(v) = hostname_value {
        let v = v.trim();
        if looks_like_container_id(v) {
            return Some(v.to_string());
        }
    }

    // /proc/self/cgroup is reliable regardless of hostname overrides.
    if let Some(id) = detect_from_cgroup(cgroup_path) {
        return Some(id);
    }

    // Last resort: /etc/hostname, accepted only when it looks like an ID.
    std::fs::read_to_string(hostname_path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| looks_like_container_id(s))
}

/// Generate a temporary container name to use while the self-update replacement starts.
pub(crate) fn temp_container_name(original: &str) -> String {
    format!("{original}-saurron-old")
}

/// Returns true if `container_id` (full 64-char ID) matches the short `own_id`
/// (typically 12 chars from `$HOSTNAME`).
pub(crate) fn is_self_container(container_id: &str, own_id: &str) -> bool {
    container_id == own_id || container_id.starts_with(own_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_from_provided_value() {
        let result =
            detect_own_container_id_inner(Some("abc123def456"), "/nonexistent", "/nonexistent");
        assert_eq!(result, Some("abc123def456".to_string()));
    }

    #[test]
    fn detect_trims_whitespace_from_value() {
        // Must be 12 hex chars after trimming to be accepted as a container ID.
        let result =
            detect_own_container_id_inner(Some("  abc123def456  "), "/nonexistent", "/nonexistent");
        assert_eq!(result, Some("abc123def456".to_string()));
    }

    #[test]
    fn detect_non_id_hostname_falls_through_to_cgroup() {
        // "saurron" is not a container ID; should fall through to cgroup detection.
        // With nonexistent paths, detection should fail.
        let result = detect_own_container_id_inner(Some("saurron"), "/nonexistent", "/nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn detect_from_hostname_file_when_no_value() {
        let path = std::env::temp_dir().join("saurron_test_hostname.txt");
        std::fs::write(&path, "abc123def456\n").unwrap();
        let result = detect_own_container_id_inner(None, path.to_str().unwrap(), "/nonexistent");
        std::fs::remove_file(&path).ok();
        assert_eq!(result, Some("abc123def456".to_string()));
    }

    #[test]
    fn detect_returns_none_when_all_missing() {
        let result = detect_own_container_id_inner(
            None,
            "/nonexistent-hostname-file-xyz",
            "/nonexistent-cgroup",
        );
        assert!(result.is_none());
    }

    #[test]
    fn non_id_hostname_file_ignored() {
        // /etc/hostname with a custom name should not be returned.
        let path = std::env::temp_dir().join("saurron_test_hostname3.txt");
        std::fs::write(&path, "  saurron  ").unwrap();
        let result = detect_own_container_id_inner(None, path.to_str().unwrap(), "/nonexistent");
        std::fs::remove_file(&path).ok();
        assert!(result.is_none());
    }

    #[test]
    fn cgroup_v1_detection() {
        let cgroup = std::env::temp_dir().join("saurron_test_cgroup_v1.txt");
        std::fs::write(
            &cgroup,
            "12:devices:/docker/7883046788d17d7be7b7812502acc23d9f97eb4487b6cc2097310a0d117d2f0a\n",
        )
        .unwrap();
        let result = detect_own_container_id_inner(
            Some("saurron"),
            "/nonexistent",
            cgroup.to_str().unwrap(),
        );
        std::fs::remove_file(&cgroup).ok();
        assert_eq!(
            result,
            Some("7883046788d17d7be7b7812502acc23d9f97eb4487b6cc2097310a0d117d2f0a".to_string())
        );
    }

    #[test]
    fn cgroup_v2_detection() {
        let cgroup = std::env::temp_dir().join("saurron_test_cgroup_v2.txt");
        std::fs::write(
            &cgroup,
            "0::/system.slice/docker-7883046788d17d7be7b7812502acc23d9f97eb4487b6cc2097310a0d117d2f0a.scope\n",
        )
        .unwrap();
        let result = detect_own_container_id_inner(
            Some("saurron"),
            "/nonexistent",
            cgroup.to_str().unwrap(),
        );
        std::fs::remove_file(&cgroup).ok();
        assert_eq!(
            result,
            Some("7883046788d17d7be7b7812502acc23d9f97eb4487b6cc2097310a0d117d2f0a".to_string())
        );
    }

    #[test]
    fn cgroup_preferred_over_hostname_file_when_hostname_env_not_id() {
        let cgroup = std::env::temp_dir().join("saurron_test_cgroup_pref.txt");
        std::fs::write(
            &cgroup,
            "12:devices:/docker/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n",
        )
        .unwrap();
        let hostname_path = std::env::temp_dir().join("saurron_test_hostname_pref.txt");
        std::fs::write(&hostname_path, "bbbbbbbbbbbb\n").unwrap();
        let result = detect_own_container_id_inner(
            Some("saurron"),
            hostname_path.to_str().unwrap(),
            cgroup.to_str().unwrap(),
        );
        std::fs::remove_file(&cgroup).ok();
        std::fs::remove_file(&hostname_path).ok();
        assert_eq!(
            result,
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string())
        );
    }

    #[test]
    fn temp_name_appends_suffix() {
        assert_eq!(temp_container_name("myapp"), "myapp-saurron-old");
        assert_eq!(temp_container_name("saurron"), "saurron-saurron-old");
    }

    #[test]
    fn is_self_container_exact_match() {
        assert!(is_self_container("abc123", "abc123"));
    }

    #[test]
    fn is_self_container_prefix_match() {
        assert!(is_self_container(
            "abc123def456789012345678901234567890123456789012345678901234",
            "abc123"
        ));
    }

    #[test]
    fn is_self_container_no_match() {
        assert!(!is_self_container("xyz999", "abc123"));
    }

    #[test]
    fn detect_own_container_id_public_wrapper_does_not_panic() {
        // Exercises the public wrapper that reads $HOSTNAME or falls back to /etc/hostname.
        let _ = detect_own_container_id();
    }
}
