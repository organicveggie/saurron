use std::str::FromStr;
use std::time::Duration;

use anyhow::Result;

use crate::{config::Config, update::parse_duration_secs};

pub enum ScheduleMode {
    RunOnce,
    Interval(Duration),
    Cron(Box<cron::Schedule>),
}

impl ScheduleMode {
    /// Returns the wall-clock time of the next scheduled run, or `None` for
    /// `RunOnce` (which runs immediately and does not repeat).
    pub fn next_run(&self) -> Option<chrono::DateTime<chrono::Local>> {
        match self {
            Self::RunOnce => None,
            Self::Interval(d) => chrono::Duration::from_std(*d)
                .ok()
                .map(|delta| chrono::Local::now() + delta),
            Self::Cron(s) => s.upcoming(chrono::Local).next(),
        }
    }
}

pub fn parse_schedule_mode(config: &Config) -> Result<ScheduleMode> {
    if config.run_once {
        if config.poll_interval.is_some() || config.schedule.is_some() {
            anyhow::bail!("--run-once is mutually exclusive with --interval and --schedule");
        }
        return Ok(ScheduleMode::RunOnce);
    }

    match (&config.poll_interval, &config.schedule) {
        (Some(_), Some(_)) => {
            anyhow::bail!("--interval and --schedule cannot be used together");
        }
        (Some(interval), None) => {
            let secs = parse_duration_secs(interval)?;
            Ok(ScheduleMode::Interval(Duration::from_secs(secs)))
        }
        (None, Some(expr)) => {
            let schedule = cron::Schedule::from_str(expr)
                .map_err(|e| anyhow::anyhow!("invalid cron expression '{}': {}", expr, e))?;
            Ok(ScheduleMode::Cron(Box::new(schedule)))
        }
        (None, None) => Ok(ScheduleMode::Interval(Duration::from_secs(86_400))),
    }
}

pub async fn run_scheduler<F, Fut>(mode: ScheduleMode, run_cycle: F)
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    match mode {
        ScheduleMode::RunOnce => {
            run_cycle().await;
        }
        ScheduleMode::Interval(duration) => loop {
            run_cycle().await;
            if let Some(next) = ScheduleMode::Interval(duration).next_run() {
                tracing::info!(next_run = %next, "next run scheduled");
            }
            tokio::time::sleep(duration).await;
        },
        ScheduleMode::Cron(schedule) => loop {
            let now = chrono::Utc::now();
            if let Some(next) = schedule.as_ref().upcoming(chrono::Utc).next() {
                let delta = next - now;
                if let Ok(wait) = delta.to_std() {
                    tokio::time::sleep(wait).await;
                }
            } else {
                tracing::warn!("cron schedule has no upcoming triggers; exiting scheduler");
                break;
            }
            run_cycle().await;
            if let Some(next) = schedule.as_ref().upcoming(chrono::Utc).next() {
                tracing::info!(next_run = %next, "next run scheduled");
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Args;
    use clap::Parser;

    fn make_config(extra_args: &[&str]) -> Config {
        let mut cmd = vec!["saurron"];
        cmd.extend_from_slice(extra_args);
        let args = Args::parse_from(cmd);
        Config::load(&args).expect("config load failed").0
    }

    #[test]
    fn run_once_mode() {
        let cfg = make_config(&["--run-once"]);
        let mode = parse_schedule_mode(&cfg).unwrap();
        assert!(matches!(mode, ScheduleMode::RunOnce));
    }

    #[test]
    fn interval_default_is_24h() {
        let cfg = make_config(&[]);
        let mode = parse_schedule_mode(&cfg).unwrap();
        match mode {
            ScheduleMode::Interval(d) => assert_eq!(d.as_secs(), 86_400),
            _ => panic!("expected Interval"),
        }
    }

    #[test]
    fn interval_from_flag() {
        let cfg = make_config(&["--interval", "5m"]);
        let mode = parse_schedule_mode(&cfg).unwrap();
        match mode {
            ScheduleMode::Interval(d) => assert_eq!(d.as_secs(), 300),
            _ => panic!("expected Interval"),
        }
    }

    #[test]
    fn interval_hours() {
        let cfg = make_config(&["--interval", "2h"]);
        let mode = parse_schedule_mode(&cfg).unwrap();
        match mode {
            ScheduleMode::Interval(d) => assert_eq!(d.as_secs(), 7_200),
            _ => panic!("expected Interval"),
        }
    }

    #[test]
    fn cron_mode_from_flag() {
        let cfg = make_config(&["--schedule", "0 */5 * * * *"]);
        let mode = parse_schedule_mode(&cfg).unwrap();
        assert!(matches!(mode, ScheduleMode::Cron(_)));
    }

    #[test]
    fn invalid_cron_expression_is_error() {
        let cfg = make_config(&["--schedule", "not-a-cron"]);
        assert!(parse_schedule_mode(&cfg).is_err());
    }

    #[test]
    fn next_run_run_once_is_none() {
        assert!(ScheduleMode::RunOnce.next_run().is_none());
    }

    #[test]
    fn next_run_interval_is_future() {
        let d = Duration::from_secs(3_600);
        let before = chrono::Utc::now();
        let next = ScheduleMode::Interval(d).next_run().unwrap();
        assert!(next > before);
        assert!(next <= before + chrono::Duration::hours(2));
    }

    #[test]
    fn next_run_cron_is_future() {
        let schedule = cron::Schedule::from_str("0 * * * * *").unwrap();
        let before = chrono::Utc::now();
        let next = ScheduleMode::Cron(Box::new(schedule)).next_run().unwrap();
        assert!(next > before);
    }

    #[test]
    fn run_once_with_interval_is_error() {
        let mut cfg = make_config(&["--run-once"]);
        cfg.poll_interval = Some("5m".to_string());
        assert!(parse_schedule_mode(&cfg).is_err());
    }

    #[test]
    fn interval_and_schedule_together_is_error() {
        let mut cfg = make_config(&["--interval", "5m"]);
        cfg.schedule = Some("0 4 * * *".to_string());
        assert!(parse_schedule_mode(&cfg).is_err());
    }

    #[tokio::test]
    async fn run_once_calls_cycle_exactly_once() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let count = Arc::new(AtomicUsize::new(0));
        let count2 = Arc::clone(&count);
        run_scheduler(ScheduleMode::RunOnce, move || {
            let c = Arc::clone(&count2);
            async move {
                c.fetch_add(1, Ordering::SeqCst);
            }
        })
        .await;
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn interval_loop_executes_cycle() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let count = Arc::new(AtomicUsize::new(0));
        let count2 = Arc::clone(&count);
        let handle = tokio::spawn(run_scheduler(
            ScheduleMode::Interval(Duration::from_millis(1)),
            move || {
                let c = Arc::clone(&count2);
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                }
            },
        ));
        tokio::time::sleep(Duration::from_millis(50)).await;
        handle.abort();
        assert!(count.load(Ordering::SeqCst) >= 1);
    }
}
