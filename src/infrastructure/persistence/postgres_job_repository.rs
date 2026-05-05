use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::error::job_repository_error::JobRepositoryError;
use crate::domain::model::job::{Job, JobKind, JobStatus};
use crate::domain::repository::job_repository::JobRepository;

#[derive(Clone)]
pub struct PostgresJobRepository {
    pool: PgPool,
}

impl PostgresJobRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct JobRow {
    id: Uuid,
    kind: String,
    status: String,
    title: String,
    session_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    error_message: Option<String>,
}

impl TryFrom<JobRow> for Job {
    type Error = JobRepositoryError;

    fn try_from(row: JobRow) -> Result<Self, Self::Error> {
        let kind = JobKind::from_db(&row.kind).ok_or(JobRepositoryError::InvalidKind(row.kind))?;

        let status =
            JobStatus::from_db(&row.status).ok_or(JobRepositoryError::InvalidStatus(row.status))?;

        Ok(Self {
            id: row.id,
            kind,
            status,
            title: row.title,
            session_id: row.session_id,
            created_at: row.created_at,
            started_at: row.started_at,
            finished_at: row.finished_at,
            error_message: row.error_message,
        })
    }
}

fn map_sqlx_error(err: sqlx::Error) -> JobRepositoryError {
    JobRepositoryError::Unexpected(err.to_string())
}

#[async_trait]
impl JobRepository for PostgresJobRepository {
    async fn save(&self, job: Job) -> Result<(), JobRepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO jobs (
              id, kind, status, title, session_id,
              created_at, started_at, finished_at, error_message
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(job.id)
        .bind(job.kind.as_str())
        .bind(job.status.as_str())
        .bind(job.title)
        .bind(job.session_id)
        .bind(job.created_at)
        .bind(job.started_at)
        .bind(job.finished_at)
        .bind(job.error_message)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Job>, JobRepositoryError> {
        let row = sqlx::query_as::<_, JobRow>(
            r#"
            SELECT
              id, kind, status, title, session_id,
              created_at, started_at, finished_at, error_message
            FROM jobs
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(TryInto::try_into).transpose()
    }

    async fn list_recent(&self, limit: i64) -> Result<Vec<Job>, JobRepositoryError> {
        let rows = sqlx::query_as::<_, JobRow>(
            r#"
            SELECT
              id, kind, status, title, session_id,
              created_at, started_at, finished_at, error_message
            FROM jobs
            ORDER BY created_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn update(&self, job: Job) -> Result<(), JobRepositoryError> {
        let result = sqlx::query(
            r#"
            UPDATE jobs
            SET
              kind = $2,
              status = $3,
              title = $4,
              session_id = $5,
              created_at = $6,
              started_at = $7,
              finished_at = $8,
              error_message = $9
            WHERE id = $1
            "#,
        )
        .bind(job.id)
        .bind(job.kind.as_str())
        .bind(job.status.as_str())
        .bind(job.title)
        .bind(job.session_id)
        .bind(job.created_at)
        .bind(job.started_at)
        .bind(job.finished_at)
        .bind(job.error_message)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        if result.rows_affected() == 0 {
            return Err(JobRepositoryError::JobNotFound(job.id));
        }

        Ok(())
    }
}
