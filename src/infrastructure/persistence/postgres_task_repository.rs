use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::error::task_repository_error::TaskRepositoryError;
use crate::domain::model::task::{Task, TaskStatus};
use crate::domain::repository::task_repository::{CreateTask, TaskRepository};

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
    request: String,
    status: String,
    session_id: Uuid,
    source_message_id: Option<Uuid>,
    parent_task_id: Option<Uuid>,
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

        Ok(Self::new(
            row.id,
            row.request,
            status,
            row.session_id,
            row.source_message_id,
            row.parent_task_id,
            row.created_at,
            row.updated_at,
            row.started_at,
            row.finished_at,
        ))
    }
}

fn map_sqlx_error(err: sqlx::Error) -> TaskRepositoryError {
    match err {
        sqlx::Error::Database(db_err) => {
            let message = db_err.message().to_string();

            if message.contains("tasks_session_id_fkey") {
                TaskRepositoryError::SessionNotFound(Uuid::nil())
            } else if message.contains("tasks_source_message_id_fkey") {
                TaskRepositoryError::MessageNotFound(Uuid::nil())
            } else if message.contains("tasks_parent_task_id_fkey") {
                TaskRepositoryError::ParentTaskNotFound(Uuid::nil())
            } else {
                TaskRepositoryError::Unexpected(message)
            }
        }
        other => TaskRepositoryError::Unexpected(other.to_string()),
    }
}

#[async_trait]
impl TaskRepository for PostgresTaskRepository {
    async fn create(&self, input: CreateTask) -> Result<Task, TaskRepositoryError> {
        if input.request.trim().is_empty() {
            return Err(TaskRepositoryError::InvalidTask(
                "task request must not be empty".to_string(),
            ));
        }

        let row = sqlx::query_as::<_, TaskRow>(
            r#"
            INSERT INTO tasks (
              request,
              session_id,
              source_message_id,
              parent_task_id
            )
            VALUES ($1, $2, $3, $4)
            RETURNING
              id,
              request,
              status,
              session_id,
              source_message_id,
              parent_task_id,
              created_at,
              updated_at,
              started_at,
              finished_at
            "#,
        )
        .bind(input.request)
        .bind(input.session_id)
        .bind(input.source_message_id)
        .bind(input.parent_task_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.try_into()
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Task>, TaskRepositoryError> {
        let row = sqlx::query_as::<_, TaskRow>(
            r#"
            SELECT
              id,
              request,
              status,
              session_id,
              source_message_id,
              parent_task_id,
              created_at,
              updated_at,
              started_at,
              finished_at
            FROM tasks
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(TryInto::try_into).transpose()
    }

    async fn list_recent(
        &self,
        status: Option<TaskStatus>,
        limit: usize,
    ) -> Result<Vec<Task>, TaskRepositoryError> {
        let limit = i64::try_from(limit)
            .map_err(|_| TaskRepositoryError::Unexpected(format!("invalid limit: {limit}")))?;

        let rows = sqlx::query_as::<_, TaskRow>(
            r#"
            SELECT
              id,
              request,
              status,
              session_id,
              source_message_id,
              parent_task_id,
              created_at,
              updated_at,
              started_at,
              finished_at
            FROM tasks
            WHERE ($1::TEXT IS NULL OR status = $1)
            ORDER BY created_at DESC, id DESC
            LIMIT $2
            "#,
        )
        .bind(status.map(TaskStatus::as_str))
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn update_status(
        &self,
        id: Uuid,
        status: TaskStatus,
    ) -> Result<Task, TaskRepositoryError> {
        let row = sqlx::query_as::<_, TaskRow>(
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
            RETURNING
              id,
              request,
              status,
              session_id,
              source_message_id,
              parent_task_id,
              created_at,
              updated_at,
              started_at,
              finished_at
            "#,
        )
        .bind(id)
        .bind(status.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.ok_or(TaskRepositoryError::NotFound(id))?.try_into()
    }

    async fn request_cancel(&self, id: Uuid) -> Result<Task, TaskRepositoryError> {
        self.update_status(id, TaskStatus::CancelRequested).await
    }

    async fn find_by_session_id(
        &self,
        session_id: Uuid,
    ) -> Result<Option<Task>, TaskRepositoryError> {
        let row = sqlx::query_as::<_, TaskRow>(
            r#"
            SELECT id, request, status, session_id, source_message_id, parent_task_id,
                created_at, updated_at, started_at, finished_at
            FROM tasks
            WHERE session_id = $1
            ORDER BY created_at DESC, id DESC
            LIMIT 1
            "#,
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(TryInto::try_into).transpose()
    }
}
