use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::error::job_run_repository_error::JobRunRepositoryError;
use crate::domain::model::job_run::{JobRun, JobRunStatus};
use crate::domain::repository::job_run_repository::JobRunRepository;

#[derive(Clone)]
pub struct PostgresJobRunRepository {
    pool: PgPool,
}

impl PostgresJobRunRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct JobRunRow {
    id: Uuid,
    job_id: Uuid,
    attempt: i32,
    status: String,
    started_at: DateTime<Utc>,
    finished_at: Option<DateTime<Utc>>,
    error_message: Option<String>,
}

impl TryFrom<JobRunRow> for JobRun {
    type Error = JobRunRepositoryError;

    fn try_from(row: JobRunRow) -> Result<Self, Self::Error> {
        let status = JobRunStatus::from_db(&row.status)
            .ok_or(JobRunRepositoryError::InvalidStatus(row.status))?;

        Ok(Self {
            id: row.id,
            job_id: row.job_id,
            attempt: row.attempt,
            status,
            started_at: row.started_at,
            finished_at: row.finished_at,
            error_message: row.error_message,
        })
    }
}

fn map_sqlx_error(err: sqlx::Error) -> JobRunRepositoryError {
    JobRunRepositoryError::Unexpected(err.to_string())
}

#[async_trait]
impl JobRunRepository for PostgresJobRunRepository {
    async fn save(&self, run: JobRun) -> Result<(), JobRunRepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO job_runs (
              id, job_id, attempt, status, started_at, finished_at, error_message
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(run.id)
        .bind(run.job_id)
        .bind(run.attempt)
        .bind(run.status.as_str())
        .bind(run.started_at)
        .bind(run.finished_at)
        .bind(run.error_message)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }

    async fn update(&self, run: JobRun) -> Result<(), JobRunRepositoryError> {
        let result = sqlx::query(
            r#"
            UPDATE job_runs
            SET
              job_id = $2,
              attempt = $3,
              status = $4,
              started_at = $5,
              finished_at = $6,
              error_message = $7
            WHERE id = $1
            "#,
        )
        .bind(run.id)
        .bind(run.job_id)
        .bind(run.attempt)
        .bind(run.status.as_str())
        .bind(run.started_at)
        .bind(run.finished_at)
        .bind(run.error_message)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        if result.rows_affected() == 0 {
            return Err(JobRunRepositoryError::JobRunNotFound(run.id));
        }

        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<JobRun>, JobRunRepositoryError> {
        let row = sqlx::query_as::<_, JobRunRow>(
            r#"
            SELECT id, job_id, attempt, status, started_at, finished_at, error_message
            FROM job_runs
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(TryInto::try_into).transpose()
    }

    async fn find_latest_by_job_id(
        &self,
        job_id: Uuid,
    ) -> Result<Option<JobRun>, JobRunRepositoryError> {
        let row = sqlx::query_as::<_, JobRunRow>(
            r#"
            SELECT id, job_id, attempt, status, started_at, finished_at, error_message
            FROM job_runs
            WHERE job_id = $1
            ORDER BY attempt DESC
            LIMIT 1
            "#,
        )
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(TryInto::try_into).transpose()
    }

    async fn next_attempt(&self, job_id: Uuid) -> Result<i32, JobRunRepositoryError> {
        let next_attempt = sqlx::query_scalar::<_, i32>(
            r#"
            SELECT COALESCE(MAX(attempt), 0) + 1
            FROM job_runs
            WHERE job_id = $1
            "#,
        )
        .bind(job_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(next_attempt)
    }
}
