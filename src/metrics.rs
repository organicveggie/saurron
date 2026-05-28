use std::sync::LazyLock;

use prometheus::{IntCounter, register_int_counter};

use crate::update::{ContainerOutcome, SessionReport};

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
    CONTAINERS_SCANNED.inc_by(report.containers.len() as u64);
    CONTAINERS_UPDATED.inc_by(
        report.containers.iter().filter(|c| c.outcome == ContainerOutcome::Updated).count() as u64,
    );
    CONTAINERS_FAILED.inc_by(
        report.containers.iter().filter(|c| c.outcome == ContainerOutcome::Failed).count() as u64,
    );
}

/// Record a cycle that was skipped because another cycle was already running.
pub fn record_skipped_cycle() {
    SCAN_CYCLES_SKIPPED.inc();
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::{ContainerOutcome, ContainerReport, SessionReport};
    use std::sync::Mutex;

    // Serialise all metric tests: the prometheus counters are process-global, so
    // concurrent tests would corrupt each other's before/after delta assertions.
    static LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn all_metrics_appear_in_prometheus_text_output() {
        let _g = LOCK.lock().unwrap();
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
        let _g = LOCK.lock().unwrap();
        let before_cycles = SCAN_CYCLES.get();
        let before_scanned = CONTAINERS_SCANNED.get();
        let before_updated = CONTAINERS_UPDATED.get();
        let before_failed = CONTAINERS_FAILED.get();

        let make = |name: &str, outcome: ContainerOutcome| ContainerReport {
            name: name.to_string(),
            outcome,
            old_image: None,
            new_image: None,
        };
        let report = SessionReport {
            containers: vec![
                make("a", ContainerOutcome::Updated),
                make("b", ContainerOutcome::Updated),
                make("c", ContainerOutcome::Skipped),
                make("d", ContainerOutcome::Failed),
                make("e", ContainerOutcome::UpToDate),
                make("f", ContainerOutcome::UpToDate),
                make("g", ContainerOutcome::UpToDate),
            ],
            ..Default::default()
        };
        record_cycle(&report);

        assert_eq!(SCAN_CYCLES.get() - before_cycles, 1);
        assert_eq!(CONTAINERS_SCANNED.get() - before_scanned, 7);
        assert_eq!(CONTAINERS_UPDATED.get() - before_updated, 2);
        assert_eq!(CONTAINERS_FAILED.get() - before_failed, 1);
    }

    #[test]
    fn record_cycle_counts_rolledback_in_scanned() {
        let _g = LOCK.lock().unwrap();
        let before_scanned = CONTAINERS_SCANNED.get();

        let report = SessionReport {
            containers: vec![ContainerReport {
                name: "x".to_string(),
                outcome: ContainerOutcome::RolledBack,
                old_image: None,
                new_image: None,
            }],
            ..Default::default()
        };
        record_cycle(&report);

        assert_eq!(CONTAINERS_SCANNED.get() - before_scanned, 1);
    }

    #[test]
    fn record_skipped_cycle_increments_by_one() {
        let _g = LOCK.lock().unwrap();
        let before = SCAN_CYCLES_SKIPPED.get();
        record_skipped_cycle();
        assert_eq!(SCAN_CYCLES_SKIPPED.get() - before, 1);
    }
}
