use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::error::event_repository_error::EventRepositoryError;
use crate::domain::model::event::Event;
use crate::domain::repository::event_repository::EventRepository;

#[derive(Clone)]
pub struct PostgresEventRepository {
    pool: PgPool,
}

impl PostgresEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct EventRow {
    id: Uuid,
    task_id: Uuid,
    event_type: String,
    payload: Value,
    created_at: DateTime<Utc>,
}

impl From<EventRow> for Event {
    fn from(row: EventRow) -> Self {
        Self {
            id: row.id,
            task_id: row.task_id,
            event_type: row.event_type,
            payload: row.payload,
            created_at: row.created_at,
        }
    }
}

fn map_sqlx_error(err: sqlx::Error) -> EventRepositoryError {
    match err {
        sqlx::Error::Database(db_err) if db_err.message().contains("events_task_id_fkey") => {
            EventRepositoryError::TaskNotFound(Uuid::nil())
        }
        other => EventRepositoryError::Unexpected(other.to_string()),
    }
}

#[async_trait]
impl EventRepository for PostgresEventRepository {
    async fn save(
        &self,
        task_id: Uuid,
        event_type: &str,
        payload: Value,
    ) -> Result<Event, EventRepositoryError> {
        let row = sqlx::query_as::<_, EventRow>(
            r#"
            INSERT INTO events (task_id, event_type, payload)
            VALUES ($1, $2, $3)
            RETURNING id, task_id, event_type, payload, created_at
            "#,
        )
        .bind(task_id)
        .bind(event_type)
        .bind(payload)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(row.into())
    }

    async fn list_for_task(&self, task_id: Uuid) -> Result<Vec<Event>, EventRepositoryError> {
        let rows = sqlx::query_as::<_, EventRow>(
            r#"
            SELECT id, task_id, event_type, payload, created_at
            FROM events
            WHERE task_id = $1
            ORDER BY created_at ASC, id ASC
            "#,
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }
}
