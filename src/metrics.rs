use std::sync::LazyLock;

use prometheus::{IntCounter, register_int_counter};

use crate::update::SessionReport;

// ── Metric definitions ────────────────────────────────────────────────────────

static SCAN_CYCLES: LazyLock<IntCounter> = LazyLock::new(|| {
    register_int_counter!(
        "saurron_scan_cycles_total",
        "Total number of completed update scan cycles"
    )
    .unwrap()
});

static SCAN_CYCLES_SKIPPED: LazyLock<IntCounter> = LazyLock::new(|| {
    register_int_counter!(
        "saurron_scan_cycles_skipped_total",
        "Total update cycles skipped because a concurrent cycle was already running"
    )
    .unwrap()
});

static CONTAINERS_SCANNED: LazyLock<IntCounter> = LazyLock::new(|| {
    register_int_counter!(
        "saurron_containers_scanned_total",
        "Total containers evaluated across all update cycles"
    )
    .unwrap()
});

static CONTAINERS_UPDATED: LazyLock<IntCounter> = LazyLock::new(|| {
    register_int_counter!(
        "saurron_containers_updated_total",
        "Total containers successfully updated to a newer image"
    )
    .unwrap()
});

static CONTAINERS_FAILED: LazyLock<IntCounter> = LazyLock::new(|| {
    register_int_counter!(
        "saurron_containers_failed_total",
        "Total containers that failed to update"
    )
    .unwrap()
});

// ── Public update functions ───────────────────────────────────────────────────

/// Record the outcome of a completed update cycle.
pub fn record_cycle(report: &SessionReport) {
    SCAN_CYCLES.inc();
    let scanned = (report.updated.len()
        + report.skipped.len()
        + report.failed.len()
        + report.rolled_back.len()
        + report.up_to_date) as u64;
    CONTAINERS_SCANNED.inc_by(scanned);
    CONTAINERS_UPDATED.inc_by(report.updated.len() as u64);
    CONTAINERS_FAILED.inc_by(report.failed.len() as u64);
}

/// Record a cycle that was skipped because another cycle was already running.
pub fn record_skipped_cycle() {
    SCAN_CYCLES_SKIPPED.inc();
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::SessionReport;

    #[test]
    fn all_metrics_appear_in_prometheus_text_output() {
        // Force initialisation of all statics.
        let _ = SCAN_CYCLES.get();
        let _ = SCAN_CYCLES_SKIPPED.get();
        let _ = CONTAINERS_SCANNED.get();
        let _ = CONTAINERS_UPDATED.get();
        let _ = CONTAINERS_FAILED.get();

        let encoder = prometheus::TextEncoder::new();
        let families = prometheus::gather();
        let mut buf = Vec::new();
        prometheus::Encoder::encode(&encoder, &families, &mut buf).unwrap();
        let text = String::from_utf8(buf).unwrap();

        assert!(text.contains("saurron_scan_cycles_total"));
        assert!(text.contains("saurron_scan_cycles_skipped_total"));
        assert!(text.contains("saurron_containers_scanned_total"));
        assert!(text.contains("saurron_containers_updated_total"));
        assert!(text.contains("saurron_containers_failed_total"));
    }

    #[test]
    fn record_cycle_increments_counters_correctly() {
        let before_cycles = SCAN_CYCLES.get();
        let before_scanned = CONTAINERS_SCANNED.get();
        let before_updated = CONTAINERS_UPDATED.get();
        let before_failed = CONTAINERS_FAILED.get();

        let report = SessionReport {
            updated: vec!["a".to_string(), "b".to_string()],
            skipped: vec!["c".to_string()],
            failed: vec!["d".to_string()],
            rolled_back: vec![],
            up_to_date: 3,
        };
        record_cycle(&report);

        assert_eq!(SCAN_CYCLES.get() - before_cycles, 1);
        assert_eq!(CONTAINERS_SCANNED.get() - before_scanned, 7); // 2+1+1+0+3
        assert_eq!(CONTAINERS_UPDATED.get() - before_updated, 2);
        assert_eq!(CONTAINERS_FAILED.get() - before_failed, 1);
    }

    #[test]
    fn record_cycle_counts_rolledback_in_scanned() {
        let before_scanned = CONTAINERS_SCANNED.get();

        let report = SessionReport {
            rolled_back: vec!["x".to_string()],
            ..Default::default()
        };
        record_cycle(&report);

        assert_eq!(CONTAINERS_SCANNED.get() - before_scanned, 1);
    }

    #[test]
    fn record_skipped_cycle_increments_by_one() {
        let before = SCAN_CYCLES_SKIPPED.get();
        record_skipped_cycle();
        assert_eq!(SCAN_CYCLES_SKIPPED.get() - before, 1);
    }
}
