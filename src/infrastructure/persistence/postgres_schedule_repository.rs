use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::error::schedule_repository_error::ScheduleRepositoryError;
use crate::domain::model::schedule::{Schedule, ScheduleRun};
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
    if input.cron.trim().is_empty() {
        return Err(ScheduleRepositoryError::InvalidSchedule(
            "cron must not be empty".to_string(),
        ));
    }
    Ok(())
}

#[async_trait]
impl ScheduleRepository for PostgresScheduleRepository {
    async fn create(&self, input: CreateSchedule) -> Result<Schedule, ScheduleRepositoryError> {
        validate_create(&input)?;

        let row = sqlx::query_as::<_, ScheduleRow>(
            r#"
            INSERT INTO schedules (title, request, cron, enabled)
            VALUES ($1, $2, $3, $4)
            RETURNING id, title, request, cron, enabled, created_at, updated_at
            "#,
        )
        .bind(input.title.trim())
        .bind(input.request.trim())
        .bind(input.cron.trim())
        .bind(input.enabled)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(row.into())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Schedule>, ScheduleRepositoryError> {
        let row = sqlx::query_as::<_, ScheduleRow>(
            r#"
            SELECT id, title, request, cron, enabled, created_at, updated_at
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
            SELECT id, title, request, cron, enabled, created_at, updated_at
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
        let row = sqlx::query_as::<_, ScheduleRow>(
            r#"
            UPDATE schedules
            SET title = COALESCE($2, title),
                request = COALESCE($3, request),
                cron = COALESCE($4, cron),
                enabled = COALESCE($5, enabled),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, title, request, cron, enabled, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(input.title.map(|v| v.trim().to_string()))
        .bind(input.request.map(|v| v.trim().to_string()))
        .bind(input.cron.map(|v| v.trim().to_string()))
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
}
