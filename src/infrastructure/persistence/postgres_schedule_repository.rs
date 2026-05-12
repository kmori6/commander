use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::error::schedule_repository_error::ScheduleRepositoryError;
use crate::domain::model::schedule::{CronExpression, Schedule, ScheduleRun, ScheduleTimezone};
use crate::domain::repository::schedule_repository::{
    CreateSchedule, ScheduleRepository, UpdateSchedule,
};

#[derive(Clone)]
pub struct PostgresScheduleRepository {
    pool: PgPool,
}

impl PostgresScheduleRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct ScheduleRow {
    id: Uuid,
    title: String,
    request: String,
    cron: String,
    timezone: String,
    enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct ScheduleRunRow {
    id: Uuid,
    schedule_id: Uuid,
    task_id: Uuid,
    scheduled_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

impl From<ScheduleRow> for Schedule {
    fn from(row: ScheduleRow) -> Self {
        Self {
            id: row.id,
            title: row.title,
            request: row.request,
            cron: row.cron,
            timezone: row.timezone,
            enabled: row.enabled,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<ScheduleRunRow> for ScheduleRun {
    fn from(row: ScheduleRunRow) -> Self {
        Self {
            id: row.id,
            schedule_id: row.schedule_id,
            task_id: row.task_id,
            scheduled_at: row.scheduled_at,
            created_at: row.created_at,
        }
    }
}

fn validate_create(input: &CreateSchedule) -> Result<(), ScheduleRepositoryError> {
    if input.title.trim().is_empty() {
        return Err(ScheduleRepositoryError::InvalidSchedule(
            "title must not be empty".to_string(),
        ));
    }

    if input.request.trim().is_empty() {
        return Err(ScheduleRepositoryError::InvalidSchedule(
            "request must not be empty".to_string(),
        ));
    }

    validate_cron(&input.cron)?;
    validate_timezone(&input.timezone)
}

fn validate_timezone(timezone: &str) -> Result<(), ScheduleRepositoryError> {
    ScheduleTimezone::parse(timezone)
        .map(|_| ())
        .map_err(ScheduleRepositoryError::InvalidSchedule)
}

fn map_sqlx_error(err: sqlx::Error) -> ScheduleRepositoryError {
    match err {
        sqlx::Error::Database(db_err)
            if db_err.message().contains("schedule_runs_task_id_fkey") =>
        {
            ScheduleRepositoryError::TaskNotFound(Uuid::nil())
        }
        other => ScheduleRepositoryError::Unexpected(other.to_string()),
    }
}

fn validate_cron(cron: &str) -> Result<(), ScheduleRepositoryError> {
    CronExpression::parse(cron)
        .map(|_| ())
        .map_err(ScheduleRepositoryError::InvalidSchedule)
}

fn validate_update(input: &UpdateSchedule) -> Result<(), ScheduleRepositoryError> {
    if let Some(title) = &input.title
        && title.trim().is_empty()
    {
        return Err(ScheduleRepositoryError::InvalidSchedule(
            "title must not be empty".to_string(),
        ));
    }

    if let Some(request) = &input.request
        && request.trim().is_empty()
    {
        return Err(ScheduleRepositoryError::InvalidSchedule(
            "request must not be empty".to_string(),
        ));
    }

    if let Some(cron) = &input.cron {
        validate_cron(cron)?;
    }

    if let Some(timezone) = &input.timezone {
        validate_timezone(timezone)?;
    }

    Ok(())
}

#[async_trait]
impl ScheduleRepository for PostgresScheduleRepository {
    async fn create(&self, input: CreateSchedule) -> Result<Schedule, ScheduleRepositoryError> {
        validate_create(&input)?;

        let row = sqlx::query_as::<_, ScheduleRow>(
            r#"
            INSERT INTO schedules (title, request, cron, timezone, enabled)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, title, request, cron, timezone, enabled, created_at, updated_at
            "#,
        )
        .bind(input.title.trim())
        .bind(input.request.trim())
        .bind(input.cron.trim())
        .bind(input.timezone.trim())
        .bind(input.enabled)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(row.into())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Schedule>, ScheduleRepositoryError> {
        let row = sqlx::query_as::<_, ScheduleRow>(
            r#"
            SELECT id, title, request, cron, timezone, enabled, created_at, updated_at
            FROM schedules
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(row.map(Into::into))
    }

    async fn list(&self) -> Result<Vec<Schedule>, ScheduleRepositoryError> {
        let rows = sqlx::query_as::<_, ScheduleRow>(
            r#"
            SELECT id, title, request, cron, timezone, enabled, created_at, updated_at
            FROM schedules
            ORDER BY updated_at DESC, id DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn update(
        &self,
        id: Uuid,
        input: UpdateSchedule,
    ) -> Result<Schedule, ScheduleRepositoryError> {
        validate_update(&input)?;

        let row = sqlx::query_as::<_, ScheduleRow>(
            r#"
            UPDATE schedules
            SET title = COALESCE($2, title),
                request = COALESCE($3, request),
                cron = COALESCE($4, cron),
                timezone = COALESCE($5, timezone),
                enabled = COALESCE($6, enabled),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, title, request, cron, timezone, enabled, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(input.title.map(|v| v.trim().to_string()))
        .bind(input.request.map(|v| v.trim().to_string()))
        .bind(input.cron.map(|v| v.trim().to_string()))
        .bind(input.timezone.map(|v| v.trim().to_string()))
        .bind(input.enabled)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(Into::into)
            .ok_or(ScheduleRepositoryError::NotFound(id))
    }

    async fn create_run(
        &self,
        schedule_id: Uuid,
        task_id: Uuid,
        scheduled_at: DateTime<Utc>,
    ) -> Result<ScheduleRun, ScheduleRepositoryError> {
        let row = sqlx::query_as::<_, ScheduleRunRow>(
            r#"
            INSERT INTO schedule_runs (schedule_id, task_id, scheduled_at)
            VALUES ($1, $2, $3)
            RETURNING id, schedule_id, task_id, scheduled_at, created_at
            "#,
        )
        .bind(schedule_id)
        .bind(task_id)
        .bind(scheduled_at)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(row.into())
    }

    async fn list_runs(
        &self,
        schedule_id: Uuid,
    ) -> Result<Vec<ScheduleRun>, ScheduleRepositoryError> {
        let rows = sqlx::query_as::<_, ScheduleRunRow>(
            r#"
            SELECT id, schedule_id, task_id, scheduled_at, created_at
            FROM schedule_runs
            WHERE schedule_id = $1
            ORDER BY scheduled_at DESC, id DESC
            "#,
        )
        .bind(schedule_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn list_enabled(&self) -> Result<Vec<Schedule>, ScheduleRepositoryError> {
        let rows = sqlx::query_as::<_, ScheduleRow>(
            r#"
        SELECT id, title, request, cron, timezone, enabled, created_at, updated_at
        FROM schedules
        WHERE enabled = TRUE
        ORDER BY id ASC
        "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_run_by_schedule_and_scheduled_at(
        &self,
        schedule_id: Uuid,
        scheduled_at: DateTime<Utc>,
    ) -> Result<Option<ScheduleRun>, ScheduleRepositoryError> {
        let row = sqlx::query_as::<_, ScheduleRunRow>(
            r#"
        SELECT id, schedule_id, task_id, scheduled_at, created_at
        FROM schedule_runs
        WHERE schedule_id = $1
          AND scheduled_at = $2
        "#,
        )
        .bind(schedule_id)
        .bind(scheduled_at)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(row.map(Into::into))
    }
}
