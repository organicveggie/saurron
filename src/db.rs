use std::path::Path;

use anyhow::Result;
use sqlx::SqlitePool;

use crate::update::{ContainerOutcome, SessionReport};

pub struct CycleRow {
    pub id: i64,
    pub started_at: String,
    pub completed_at: String,
    pub trigger: String,
    pub updated: i64,
    pub rolled_back: i64,
    pub failed: i64,
    pub skipped: i64,
    pub up_to_date: i64,
}

pub struct CycleContainerRow {
    pub id: i64,
    pub cycle_id: i64,
    pub name: String,
    pub old_image: Option<String>,
    pub new_image: Option<String>,
    pub outcome: String,
}

pub async fn init_pool(path: &Path) -> Result<SqlitePool> {
    let url = format!("sqlite://{}?mode=rwc", path.display());
    let pool = SqlitePool::connect(&url).await?;
    sqlx::migrate!().run(&pool).await?;
    Ok(pool)
}

pub async fn record_cycle(pool: &SqlitePool, report: &SessionReport, trigger: &str) -> Result<i64> {
    let started_at = report.started_at.to_rfc3339();
    let completed_at = report.completed_at.to_rfc3339();

    let updated =
        report.containers.iter().filter(|c| c.outcome == ContainerOutcome::Updated).count() as i64;
    let rolled_back = report
        .containers
        .iter()
        .filter(|c| c.outcome == ContainerOutcome::RolledBack)
        .count() as i64;
    let failed =
        report.containers.iter().filter(|c| c.outcome == ContainerOutcome::Failed).count() as i64;
    let skipped =
        report.containers.iter().filter(|c| c.outcome == ContainerOutcome::Skipped).count() as i64;
    let up_to_date = report
        .containers
        .iter()
        .filter(|c| c.outcome == ContainerOutcome::UpToDate)
        .count() as i64;

    let cycle_id = sqlx::query!(
        "INSERT INTO cycles (started_at, completed_at, trigger, updated, rolled_back, failed, skipped, up_to_date) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        started_at,
        completed_at,
        trigger,
        updated,
        rolled_back,
        failed,
        skipped,
        up_to_date,
    )
    .execute(pool)
    .await?
    .last_insert_rowid();

    for container in &report.containers {
        let outcome = match container.outcome {
            ContainerOutcome::Updated => "updated",
            ContainerOutcome::RolledBack => "rolled_back",
            ContainerOutcome::Failed => "failed",
            ContainerOutcome::Skipped => "skipped",
            ContainerOutcome::UpToDate => "up_to_date",
        };
        sqlx::query!(
            "INSERT INTO cycle_containers (cycle_id, name, old_image, new_image, outcome) \
             VALUES (?, ?, ?, ?, ?)",
            cycle_id,
            container.name,
            container.old_image,
            container.new_image,
            outcome,
        )
        .execute(pool)
        .await?;
    }

    Ok(cycle_id)
}

pub async fn list_cycles(
    pool: &SqlitePool,
    page: i64,
    per_page: i64,
) -> Result<(Vec<CycleRow>, i64)> {
    let offset = (page - 1) * per_page;
    let total: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM cycles")
        .fetch_one(pool)
        .await?;
    let rows = sqlx::query_as!(
        CycleRow,
        "SELECT id, started_at, completed_at, trigger, updated, rolled_back, failed, skipped, up_to_date \
         FROM cycles ORDER BY started_at DESC LIMIT ? OFFSET ?",
        per_page,
        offset,
    )
    .fetch_all(pool)
    .await?;
    Ok((rows, total))
}

pub async fn get_cycle(
    pool: &SqlitePool,
    id: i64,
) -> Result<Option<(CycleRow, Vec<CycleContainerRow>)>> {
    let cycle = sqlx::query_as!(
        CycleRow,
        "SELECT id, started_at, completed_at, trigger, updated, rolled_back, failed, skipped, up_to_date \
         FROM cycles WHERE id = ?",
        id,
    )
    .fetch_optional(pool)
    .await?;

    let Some(cycle) = cycle else {
        return Ok(None);
    };

    let containers = sqlx::query_as!(
        CycleContainerRow,
        "SELECT id, cycle_id, name, old_image, new_image, outcome \
         FROM cycle_containers WHERE cycle_id = ? ORDER BY id",
        id,
    )
    .fetch_all(pool)
    .await?;

    Ok(Some((cycle, containers)))
}
