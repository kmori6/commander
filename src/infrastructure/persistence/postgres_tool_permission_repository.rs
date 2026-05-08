use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::error::tool_permission_repository_error::ToolPermissionRepositoryError;
use crate::domain::model::tool_call::{ToolPermission, ToolPermissionMode};
use crate::domain::repository::tool_permission_repository::ToolPermissionRepository;

#[derive(Clone)]
pub struct PostgresToolPermissionRepository {
    pool: PgPool,
}

impl PostgresToolPermissionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct ToolPermissionRow {
    id: Uuid,
    tool_name: String,
    mode: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<ToolPermissionRow> for ToolPermission {
    type Error = ToolPermissionRepositoryError;

    fn try_from(row: ToolPermissionRow) -> Result<Self, Self::Error> {
        let mode = ToolPermissionMode::from_db(&row.mode).ok_or_else(|| {
            ToolPermissionRepositoryError::Unexpected(format!(
                "unknown tool permission mode: {}",
                row.mode
            ))
        })?;

        Ok(Self {
            id: row.id,
            tool_name: row.tool_name,
            mode,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

fn map_sqlx_error(err: sqlx::Error) -> ToolPermissionRepositoryError {
    ToolPermissionRepositoryError::Unexpected(err.to_string())
}

#[async_trait]
impl ToolPermissionRepository for PostgresToolPermissionRepository {
    async fn list(&self) -> Result<Vec<ToolPermission>, ToolPermissionRepositoryError> {
        let rows = sqlx::query_as::<_, ToolPermissionRow>(
            r#"
            SELECT id, tool_name, mode, created_at, updated_at
            FROM tool_permissions
            ORDER BY tool_name ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn upsert(
        &self,
        tool_name: &str,
        mode: ToolPermissionMode,
    ) -> Result<ToolPermission, ToolPermissionRepositoryError> {
        let tool_name = tool_name.trim();

        if tool_name.is_empty() {
            return Err(ToolPermissionRepositoryError::InvalidPermission(
                "tool_name must not be empty".to_string(),
            ));
        }

        let row = sqlx::query_as::<_, ToolPermissionRow>(
            r#"
            INSERT INTO tool_permissions (tool_name, mode)
            VALUES ($1, $2)
            ON CONFLICT (tool_name)
            DO UPDATE SET
              mode = EXCLUDED.mode,
              updated_at = NOW()
            RETURNING id, tool_name, mode, created_at, updated_at
            "#,
        )
        .bind(tool_name)
        .bind(mode.as_str())
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.try_into()
    }

    async fn find_by_tool_name(
        &self,
        tool_name: &str,
    ) -> Result<Option<ToolPermission>, ToolPermissionRepositoryError> {
        let row = sqlx::query_as::<_, ToolPermissionRow>(
            r#"
            SELECT id, tool_name, mode, created_at, updated_at
            FROM tool_permissions
            WHERE tool_name = $1
            "#,
        )
        .bind(tool_name)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(TryInto::try_into).transpose()
    }
}
