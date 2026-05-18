use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::error::task_repository_error::TaskRepositoryError;
use crate::domain::model::task::{Task, TaskSourceKind, TaskStatus};
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
    session_id: Option<Uuid>,
    source_kind: String,
    source_message_id: Option<Uuid>,
    source_schedule_id: Option<Uuid>,
    source_tool_call_id: Option<String>,
    subagent_profile: Option<String>,
    parent_task_id: Option<Uuid>,
    scheduled_at: Option<DateTime<Utc>>,
    output: String,
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

        let source_kind = TaskSourceKind::from_db(&row.source_kind).ok_or_else(|| {
            TaskRepositoryError::Unexpected(format!(
                "unknown task source kind: {}",
                row.source_kind
            ))
        })?;

        Ok(Self::new(
            row.id,
            row.request,
            status,
            row.session_id,
            source_kind,
            row.source_message_id,
            row.source_schedule_id,
            row.source_tool_call_id,
            row.subagent_profile,
            row.parent_task_id,
            row.scheduled_at,
            row.output,
            row.error,
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

const TASK_COLUMNS: &str = r#"
id,
request,
status,
session_id,
source_kind,
source_message_id,
source_schedule_id,
source_tool_call_id,
subagent_profile,
parent_task_id,
scheduled_at,
output,
error,
created_at,
updated_at,
started_at,
finished_at
"#;

#[async_trait]
impl TaskRepository for PostgresTaskRepository {
    async fn create(&self, input: CreateTask) -> Result<Task, TaskRepositoryError> {
        if input.request.trim().is_empty() {
            return Err(TaskRepositoryError::InvalidTask(
                "task request must not be empty".to_string(),
            ));
        }

        let row = sqlx::query_as::<_, TaskRow>(&format!(
            r#"
            INSERT INTO tasks (
              request,
              session_id,
              source_kind,
              source_message_id,
              source_schedule_id,
              source_tool_call_id,
              subagent_profile,
              parent_task_id,
              scheduled_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING {TASK_COLUMNS}
            "#
        ))
        .bind(input.request)
        .bind(input.session_id)
        .bind(input.source_kind.as_str())
        .bind(input.source_message_id)
        .bind(input.source_schedule_id)
        .bind(input.source_tool_call_id)
        .bind(input.subagent_profile)
        .bind(input.parent_task_id)
        .bind(input.scheduled_at)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.try_into()
    }

    async fn complete(&self, id: Uuid, output: String) -> Result<Task, TaskRepositoryError> {
        let row = sqlx::query_as::<_, TaskRow>(&format!(
            r#"
            UPDATE tasks
            SET status = 'completed',
                output = $2,
                error = NULL,
                updated_at = NOW(),
                started_at = COALESCE(started_at, NOW()),
                finished_at = NOW()
            WHERE id = $1
            RETURNING {TASK_COLUMNS}
            "#
        ))
        .bind(id)
        .bind(output)
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

    async fn cancel(&self, id: Uuid) -> Result<Task, TaskRepositoryError> {
        self.update_status(id, TaskStatus::Cancelled).await
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

    async fn request_cancel(&self, id: Uuid) -> Result<Task, TaskRepositoryError> {
        self.update_status(id, TaskStatus::CancelRequested).await
    }

    async fn cancel_children(&self, parent_task_id: Uuid) -> Result<u64, TaskRepositoryError> {
        let result = sqlx::query(
            r#"
            UPDATE tasks
            SET status = CASE
                WHEN status IN ('queued', 'awaiting_approval') THEN 'cancelled'
                WHEN status = 'running' THEN 'cancel_requested'
                ELSE status
                END,
                updated_at = NOW(),
                finished_at = CASE
                WHEN status IN ('queued', 'awaiting_approval') THEN NOW()
                ELSE finished_at
                END
            WHERE parent_task_id = $1
            AND status IN ('queued', 'running', 'awaiting_approval')
            "#,
        )
        .bind(parent_task_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(result.rows_affected())
    }

    async fn has_open_children(&self, parent_task_id: Uuid) -> Result<bool, TaskRepositoryError> {
        let has_open_children = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
              SELECT 1
              FROM tasks
              WHERE parent_task_id = $1
                AND status NOT IN ('completed', 'failed', 'cancelled')
            )
            "#,
        )
        .bind(parent_task_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(has_open_children)
    }

    async fn list_joinable_children(&self, limit: usize) -> Result<Vec<Task>, TaskRepositoryError> {
        let limit = i64::try_from(limit)
            .map_err(|_| TaskRepositoryError::Unexpected(format!("invalid limit: {limit}")))?;

        let rows = sqlx::query_as::<_, TaskRow>(&format!(
            r#"
            WITH ready AS (
            SELECT DISTINCT ON (child.parent_task_id, child.source_tool_call_id)
                child.id
            FROM tasks child
            JOIN tasks parent ON parent.id = child.parent_task_id
            WHERE child.source_tool_call_id IS NOT NULL
                AND parent.status IN ('awaiting_child', 'cancel_requested')
                AND NOT EXISTS (
                SELECT 1
                FROM tasks open_child
                WHERE open_child.parent_task_id = child.parent_task_id
                    AND open_child.source_tool_call_id = child.source_tool_call_id
                    AND open_child.status NOT IN ('completed', 'failed', 'cancelled')
                )
            ORDER BY child.parent_task_id, child.source_tool_call_id, child.created_at ASC, child.id ASC
            LIMIT $1
            )
            SELECT {TASK_COLUMNS}
            FROM tasks
            WHERE id IN (SELECT id FROM ready)
            ORDER BY created_at ASC, id ASC
            "#
        ))
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn list_by_source_schedule_id(
        &self,
        schedule_id: Uuid,
    ) -> Result<Vec<Task>, TaskRepositoryError> {
        let rows = sqlx::query_as::<_, TaskRow>(&format!(
            r#"
            SELECT {TASK_COLUMNS}
            FROM tasks
            WHERE source_kind = 'schedule'
            AND source_schedule_id = $1
            ORDER BY scheduled_at DESC NULLS LAST, created_at DESC, id DESC
            "#
        ))
        .bind(schedule_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn find_by_source_schedule_id_and_scheduled_at(
        &self,
        schedule_id: Uuid,
        scheduled_at: DateTime<Utc>,
    ) -> Result<Option<Task>, TaskRepositoryError> {
        let row = sqlx::query_as::<_, TaskRow>(&format!(
            r#"
            SELECT {TASK_COLUMNS}
            FROM tasks
            WHERE source_kind = 'schedule'
            AND source_schedule_id = $1
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

    async fn list_children(
        &self,
        parent_task_id: Uuid,
        status: Option<TaskStatus>,
        limit: usize,
    ) -> Result<Vec<Task>, TaskRepositoryError> {
        let limit = i64::try_from(limit)
            .map_err(|_| TaskRepositoryError::Unexpected(format!("invalid limit: {limit}")))?;

        let rows = sqlx::query_as::<_, TaskRow>(&format!(
            r#"
            SELECT {TASK_COLUMNS}
            FROM tasks
            WHERE parent_task_id = $1
            AND ($2::TEXT IS NULL OR status = $2)
            ORDER BY created_at ASC, id ASC
            LIMIT $3
            "#
        ))
        .bind(parent_task_id)
        .bind(status.map(TaskStatus::as_str))
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn list_child_group(
        &self,
        parent_task_id: Uuid,
        source_tool_call_id: &str,
    ) -> Result<Vec<Task>, TaskRepositoryError> {
        let rows = sqlx::query_as::<_, TaskRow>(&format!(
            r#"
            SELECT {TASK_COLUMNS}
            FROM tasks
            WHERE parent_task_id = $1
              AND source_tool_call_id = $2
            ORDER BY created_at ASC, id ASC
            "#
        ))
        .bind(parent_task_id)
        .bind(source_tool_call_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(TryInto::try_into).collect()
    }
}
