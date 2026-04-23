/// Detect Saurron's own Docker container ID.
///
/// Docker sets `$HOSTNAME` to the first 12 characters of the container ID.
/// Falls back to reading `/etc/hostname` when the env var is absent or empty.
/// Returns `None` when running outside a container or when detection fails.
pub(crate) fn detect_own_container_id() -> Option<String> {
    let hostname_env = std::env::var("HOSTNAME").ok();
    detect_own_container_id_inner(hostname_env.as_deref(), "/etc/hostname")
}

fn detect_own_container_id_inner(
    hostname_value: Option<&str>,
    hostname_path: &str,
) -> Option<String> {
    if let Some(v) = hostname_value {
        let v = v.trim();
        if !v.is_empty() {
            return Some(v.to_string());
        }
    }
    std::fs::read_to_string(hostname_path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
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
        let result = detect_own_container_id_inner(Some("abc123def456"), "/nonexistent");
        assert_eq!(result, Some("abc123def456".to_string()));
    }

    #[test]
    fn detect_trims_whitespace_from_value() {
        let result = detect_own_container_id_inner(Some("  abc123  "), "/nonexistent");
        assert_eq!(result, Some("abc123".to_string()));
    }

    #[test]
    fn detect_from_hostname_file_when_no_value() {
        let path = std::env::temp_dir().join("saurron_test_hostname.txt");
        std::fs::write(&path, "abc123def456\n").unwrap();
        let result = detect_own_container_id_inner(None, path.to_str().unwrap());
        std::fs::remove_file(&path).ok();
        assert_eq!(result, Some("abc123def456".to_string()));
    }

    #[test]
    fn detect_returns_none_when_both_missing() {
        let result = detect_own_container_id_inner(None, "/nonexistent-hostname-file-xyz");
        assert!(result.is_none());
    }

    #[test]
    fn empty_value_falls_back_to_file() {
        let path = std::env::temp_dir().join("saurron_test_hostname2.txt");
        std::fs::write(&path, "  containerid  ").unwrap();
        let result = detect_own_container_id_inner(Some(""), path.to_str().unwrap());
        std::fs::remove_file(&path).ok();
        assert_eq!(result, Some("containerid".to_string()));
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
