use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::error::task_result_repository_error::TaskResultRepositoryError;
use crate::domain::model::task_result::{TaskResult, TaskResultStatus};
use crate::domain::repository::task_result_repository::TaskResultRepository;

#[derive(Clone)]
pub struct PostgresTaskResultRepository {
    pool: PgPool,
}

impl PostgresTaskResultRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct ResultRow {
    id: Uuid,
    task_id: Uuid,
    status: String,
    output: String,
    created_at: DateTime<Utc>,
}

impl TryFrom<ResultRow> for TaskResult {
    type Error = TaskResultRepositoryError;

    fn try_from(row: ResultRow) -> Result<Self, Self::Error> {
        let status = TaskResultStatus::from_db(&row.status).ok_or_else(|| {
            TaskResultRepositoryError::Unexpected(format!("unknown result status: {}", row.status))
        })?;

        Ok(Self {
            id: row.id,
            task_id: row.task_id,
            status,
            output: row.output,
            created_at: row.created_at,
        })
    }
}

fn map_sqlx_error(err: sqlx::Error) -> TaskResultRepositoryError {
    match err {
        sqlx::Error::Database(db_err) if db_err.message().contains("results_task_id_fkey") => {
            TaskResultRepositoryError::TaskNotFound(Uuid::nil())
        }
        other => TaskResultRepositoryError::Unexpected(other.to_string()),
    }
}

#[async_trait]
impl TaskResultRepository for PostgresTaskResultRepository {
    async fn save(
        &self,
        task_id: Uuid,
        status: TaskResultStatus,
        output: String,
    ) -> Result<TaskResult, TaskResultRepositoryError> {
        let row = sqlx::query_as::<_, ResultRow>(
            r#"
            INSERT INTO results (task_id, status, output)
            VALUES ($1, $2, $3)
            ON CONFLICT (task_id)
            DO UPDATE SET status = EXCLUDED.status, output = EXCLUDED.output
            RETURNING id, task_id, status, output, created_at
            "#,
        )
        .bind(task_id)
        .bind(status.as_str())
        .bind(output)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.try_into()
    }

    async fn find_by_task_id(
        &self,
        task_id: Uuid,
    ) -> Result<Option<TaskResult>, TaskResultRepositoryError> {
        let row = sqlx::query_as::<_, ResultRow>(
            r#"
            SELECT id, task_id, status, output, created_at
            FROM results
            WHERE task_id = $1
            "#,
        )
        .bind(task_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(TryInto::try_into).transpose()
    }
}
