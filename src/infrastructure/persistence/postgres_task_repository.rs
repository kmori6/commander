use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::error::task_repository_error::TaskRepositoryError;
use crate::domain::model::task::{Task, TaskStatus};
use crate::domain::repository::task_repository::TaskRepository;

#[derive(Clone)]
pub struct PostgresTaskRepository {
    pool: PgPool,
}

impl PostgresTaskRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct TaskRow {
    id: Uuid,
    status: String,
    session_id: Option<Uuid>,
    schedule_id: Option<Uuid>,
    scheduled_at: Option<DateTime<Utc>>,
    error: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
}

impl TryFrom<TaskRow> for Task {
    type Error = TaskRepositoryError;

    fn try_from(row: TaskRow) -> Result<Self, Self::Error> {
        let status = TaskStatus::from_db(&row.status).ok_or_else(|| {
            TaskRepositoryError::Unexpected(format!("unknown task status: {}", row.status))
        })?;

        Ok(Task {
            id: row.id,
            status,
            session_id: row.session_id,
            schedule_id: row.schedule_id,
            scheduled_at: row.scheduled_at,
            error: row.error,
            created_at: row.created_at,
            updated_at: row.updated_at,
            started_at: row.started_at,
            finished_at: row.finished_at,
        })
    }
}

fn map_sqlx_error(err: sqlx::Error) -> TaskRepositoryError {
    match err {
        sqlx::Error::Database(db_err) => {
            let message = db_err.message().to_string();

            if message.contains("tasks_session_id_fkey") {
                TaskRepositoryError::SessionNotFound(Uuid::nil())
            } else {
                TaskRepositoryError::Unexpected(message)
            }
        }
        other => TaskRepositoryError::Unexpected(other.to_string()),
    }
}

const TASK_COLUMNS: &str = r#"
id,
status,
session_id,
schedule_id,
scheduled_at,
error,
created_at,
updated_at,
started_at,
finished_at
"#;

#[async_trait]
impl TaskRepository for PostgresTaskRepository {
    async fn create(
        &self,
        session_id: Option<Uuid>,
        schedule_id: Option<Uuid>,
        scheduled_at: Option<DateTime<Utc>>,
    ) -> Result<Task, TaskRepositoryError> {
        let row = sqlx::query_as::<_, TaskRow>(&format!(
            r#"
            INSERT INTO tasks (
              session_id,
              schedule_id,
              scheduled_at
            )
            VALUES ($1, $2, $3)
            RETURNING {TASK_COLUMNS}
            "#
        ))
        .bind(session_id)
        .bind(schedule_id)
        .bind(scheduled_at)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.try_into()
    }

    async fn complete(&self, id: Uuid) -> Result<Task, TaskRepositoryError> {
        let row = sqlx::query_as::<_, TaskRow>(&format!(
            r#"
            UPDATE tasks
            SET status = 'completed',
                error = NULL,
                updated_at = NOW(),
                started_at = COALESCE(started_at, NOW()),
                finished_at = NOW()
            WHERE id = $1
            RETURNING {TASK_COLUMNS}
            "#
        ))
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.ok_or(TaskRepositoryError::NotFound(id))?.try_into()
    }

    async fn fail(&self, id: Uuid, error: String) -> Result<Task, TaskRepositoryError> {
        let row = sqlx::query_as::<_, TaskRow>(&format!(
            r#"
            UPDATE tasks
            SET status = 'failed',
                error = $2,
                updated_at = NOW(),
                started_at = COALESCE(started_at, NOW()),
                finished_at = NOW()
            WHERE id = $1
            RETURNING {TASK_COLUMNS}
            "#
        ))
        .bind(id)
        .bind(error)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.ok_or(TaskRepositoryError::NotFound(id))?.try_into()
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Task>, TaskRepositoryError> {
        let row = sqlx::query_as::<_, TaskRow>(&format!(
            r#"
            SELECT {TASK_COLUMNS}
            FROM tasks
            WHERE id = $1
            "#
        ))
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(TryInto::try_into).transpose()
    }

    async fn list_by_session_id(&self, session_id: Uuid) -> Result<Vec<Task>, TaskRepositoryError> {
        let rows = sqlx::query_as::<_, TaskRow>(&format!(
            r#"
            SELECT {TASK_COLUMNS}
            FROM tasks
            WHERE session_id = $1
            ORDER BY created_at ASC, id ASC
            "#
        ))
        .bind(session_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn list_recent(
        &self,
        status: Option<TaskStatus>,
        limit: usize,
    ) -> Result<Vec<Task>, TaskRepositoryError> {
        let limit = i64::try_from(limit)
            .map_err(|_| TaskRepositoryError::Unexpected(format!("invalid limit: {limit}")))?;

        let rows = sqlx::query_as::<_, TaskRow>(&format!(
            r#"
            SELECT {TASK_COLUMNS}
            FROM tasks
            WHERE ($1::TEXT IS NULL OR status = $1)
            ORDER BY created_at DESC, id DESC
            LIMIT $2
            "#
        ))
        .bind(status.map(TaskStatus::as_str))
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn claim_queued(&self, limit: usize) -> Result<Vec<Task>, TaskRepositoryError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let limit = i64::try_from(limit)
            .map_err(|_| TaskRepositoryError::Unexpected(format!("invalid limit: {limit}")))?;

        let rows = sqlx::query_as::<_, TaskRow>(&format!(
            r#"
            WITH claimed AS (
                SELECT id
                FROM tasks
                WHERE status = 'queued'
                ORDER BY created_at ASC, id ASC
                LIMIT $1
                FOR UPDATE SKIP LOCKED
            ),
            updated AS (
                UPDATE tasks
                SET status = 'running',
                    updated_at = NOW(),
                    started_at = COALESCE(started_at, NOW())
                WHERE id IN (SELECT id FROM claimed)
                RETURNING {TASK_COLUMNS}
            )
            SELECT {TASK_COLUMNS}
            FROM updated
            ORDER BY created_at ASC, id ASC
            "#
        ))
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn requeue_running(&self) -> Result<u64, TaskRepositoryError> {
        let result = sqlx::query(
            r#"
            UPDATE tasks
            SET status = 'queued',
                updated_at = NOW(),
                started_at = NULL
            WHERE status = 'running'
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(result.rows_affected())
    }

    async fn update_status(
        &self,
        id: Uuid,
        status: TaskStatus,
    ) -> Result<Task, TaskRepositoryError> {
        let row = sqlx::query_as::<_, TaskRow>(&format!(
            r#"
            UPDATE tasks
            SET status = $2,
                updated_at = NOW(),
                started_at = CASE
                  WHEN $2 = 'running' AND started_at IS NULL THEN NOW()
                  ELSE started_at
                END,
                finished_at = CASE
                  WHEN $2 IN ('completed', 'failed', 'cancelled') THEN NOW()
                  ELSE finished_at
                END
            WHERE id = $1
            RETURNING {TASK_COLUMNS}
            "#
        ))
        .bind(id)
        .bind(status.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.ok_or(TaskRepositoryError::NotFound(id))?.try_into()
    }

    async fn list_by_schedule_id(
        &self,
        schedule_id: Uuid,
    ) -> Result<Vec<Task>, TaskRepositoryError> {
        let rows = sqlx::query_as::<_, TaskRow>(&format!(
            r#"
            SELECT {TASK_COLUMNS}
            FROM tasks
            WHERE schedule_id = $1
            ORDER BY scheduled_at DESC NULLS LAST, created_at DESC, id DESC
            "#
        ))
        .bind(schedule_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn find_by_schedule_id_and_scheduled_at(
        &self,
        schedule_id: Uuid,
        scheduled_at: DateTime<Utc>,
    ) -> Result<Option<Task>, TaskRepositoryError> {
        let row = sqlx::query_as::<_, TaskRow>(&format!(
            r#"
            SELECT {TASK_COLUMNS}
            FROM tasks
            WHERE schedule_id = $1
            AND scheduled_at = $2
            "#
        ))
        .bind(schedule_id)
        .bind(scheduled_at)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(TryInto::try_into).transpose()
    }
}
