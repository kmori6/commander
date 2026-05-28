use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::domain::error::task_repository_error::TaskRepositoryError;
use crate::domain::model::task::{Task, TaskSource, TaskStatus};
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
        let status = task_status_from_db(&row.status).ok_or_else(|| {
            TaskRepositoryError::Unexpected(format!("unknown task status: {}", row.status))
        })?;

        let source = task_source_from_columns(row.session_id, row.schedule_id, row.scheduled_at)?;

        Ok(Task {
            id: row.id,
            source,
            status,
            error: row.error,
            created_at: row.created_at,
            updated_at: row.updated_at,
            started_at: row.started_at,
            finished_at: row.finished_at,
        })
    }
}

fn task_status_from_db(value: &str) -> Option<TaskStatus> {
    match value {
        "queued" => Some(TaskStatus::Queued),
        "running" => Some(TaskStatus::Running),
        "awaiting_approval" => Some(TaskStatus::AwaitingApproval),
        "completed" => Some(TaskStatus::Completed),
        "failed" => Some(TaskStatus::Failed),
        "cancelled" => Some(TaskStatus::Cancelled),
        _ => None,
    }
}

fn task_source_from_columns(
    session_id: Option<Uuid>,
    schedule_id: Option<Uuid>,
    scheduled_at: Option<DateTime<Utc>>,
) -> Result<TaskSource, TaskRepositoryError> {
    match (session_id, schedule_id, scheduled_at) {
        (None, None, None) => Ok(TaskSource::Direct),
        (Some(session_id), None, None) => Ok(TaskSource::Session { session_id }),
        (None, Some(schedule_id), Some(scheduled_at)) => Ok(TaskSource::Schedule {
            schedule_id,
            scheduled_at,
        }),
        (None, None, Some(scheduled_at)) => Ok(TaskSource::Watch { scheduled_at }),
        _ => Err(TaskRepositoryError::Unexpected(
            "invalid task source: session_id, schedule_id, and scheduled_at combination is invalid"
                .to_string(),
        )),
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

async fn lock_task(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<Task, TaskRepositoryError> {
    let row = sqlx::query_as::<_, TaskRow>(&format!(
        r#"
        SELECT {TASK_COLUMNS}
        FROM tasks
        WHERE id = $1
        FOR UPDATE
        "#
    ))
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;

    row.ok_or(TaskRepositoryError::NotFound(id))?.try_into()
}

async fn persist_task(
    tx: &mut Transaction<'_, Postgres>,
    task: &Task,
) -> Result<Task, TaskRepositoryError> {
    let row = sqlx::query_as::<_, TaskRow>(&format!(
        r#"
        UPDATE tasks
        SET status = $2,
            error = $3,
            updated_at = $4,
            started_at = $5,
            finished_at = $6
        WHERE id = $1
        RETURNING {TASK_COLUMNS}
        "#
    ))
    .bind(task.id)
    .bind(task.status.as_str())
    .bind(task.error.as_deref())
    .bind(task.updated_at)
    .bind(task.started_at)
    .bind(task.finished_at)
    .fetch_one(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;

    row.try_into()
}

impl PostgresTaskRepository {
    async fn update_task<F>(&self, id: Uuid, mutate: F) -> Result<Task, TaskRepositoryError>
    where
        F: FnOnce(&mut Task, DateTime<Utc>) -> Result<(), String>,
    {
        let mut tx = self.pool.begin().await.map_err(map_sqlx_error)?;
        let mut task = lock_task(&mut tx, id).await?;

        mutate(&mut task, Utc::now())?;

        let task = persist_task(&mut tx, &task).await?;
        tx.commit().await.map_err(map_sqlx_error)?;
        Ok(task)
    }
}

#[async_trait]
impl TaskRepository for PostgresTaskRepository {
    async fn create(&self, source: TaskSource) -> Result<Task, TaskRepositoryError> {
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
        .bind(source.session_id())
        .bind(source.schedule_id())
        .bind(source.scheduled_at())
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.try_into()
    }

    async fn start(&self, id: Uuid) -> Result<Task, TaskRepositoryError> {
        self.update_task(id, |task, now| task.start(now)).await
    }

    async fn await_approval(&self, id: Uuid) -> Result<Task, TaskRepositoryError> {
        self.update_task(id, |task, now| task.await_approval(now))
            .await
    }

    async fn resume_after_approval(&self, id: Uuid) -> Result<Task, TaskRepositoryError> {
        self.update_task(id, |task, now| task.resume_after_approval(now))
            .await
    }

    async fn complete(&self, id: Uuid) -> Result<Task, TaskRepositoryError> {
        self.update_task(id, |task, now| task.complete(now)).await
    }

    async fn fail(&self, id: Uuid, error: String) -> Result<Task, TaskRepositoryError> {
        self.update_task(id, |task, now| task.fail(error, now))
            .await
    }

    async fn cancel(&self, id: Uuid) -> Result<Task, TaskRepositoryError> {
        self.update_task(id, |task, now| task.cancel(now)).await
    }

    async fn claim_queued(&self, limit: usize) -> Result<Vec<Task>, TaskRepositoryError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let limit = i64::try_from(limit)
            .map_err(|_| TaskRepositoryError::Unexpected(format!("invalid limit: {limit}")))?;

        let mut tx = self.pool.begin().await.map_err(map_sqlx_error)?;

        let rows = sqlx::query_as::<_, TaskRow>(&format!(
            r#"
            SELECT {TASK_COLUMNS}
            FROM tasks
            WHERE status = 'queued'
            ORDER BY created_at ASC, id ASC
            LIMIT $1
            FOR UPDATE SKIP LOCKED
            "#
        ))
        .bind(limit)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sqlx_error)?;

        let now = Utc::now();
        let mut tasks = Vec::new();

        for row in rows {
            let mut task: Task = row.try_into()?;
            task.start(now)?;
            tasks.push(persist_task(&mut tx, &task).await?);
        }

        tx.commit().await.map_err(map_sqlx_error)?;
        Ok(tasks)
    }

    async fn requeue_running(&self) -> Result<u64, TaskRepositoryError> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx_error)?;

        let rows = sqlx::query_as::<_, TaskRow>(&format!(
            r#"
            SELECT {TASK_COLUMNS}
            FROM tasks
            WHERE status = 'running'
            FOR UPDATE
            "#
        ))
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sqlx_error)?;

        let now = Utc::now();
        let count = rows.len() as u64;

        for row in rows {
            let mut task: Task = row.try_into()?;
            task.recover_interrupted(now)?;
            persist_task(&mut tx, &task).await?;
        }

        tx.commit().await.map_err(map_sqlx_error)?;
        Ok(count)
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

    async fn list_runs(&self, schedule_id: Uuid) -> Result<Vec<Task>, TaskRepositoryError> {
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

    async fn find_run(
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
