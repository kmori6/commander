use async_trait::async_trait;
use sqlx::PgPool;

use crate::domain::error::tool_execution_rule_repository_error::ToolExecutionRuleRepositoryError;
use crate::domain::model::tool_execution_rule::{ToolExecutionRule, ToolExecutionRuleAction};
use crate::domain::repository::tool_execution_rule_repository::ToolExecutionRuleRepository;

#[derive(Clone)]
pub struct PostgresToolExecutionRuleRepository {
    pool: PgPool,
}

impl PostgresToolExecutionRuleRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct ToolExecutionRuleRow {
    tool_name: String,
    action: String,
}

impl TryFrom<ToolExecutionRuleRow> for ToolExecutionRule {
    type Error = ToolExecutionRuleRepositoryError;

    fn try_from(row: ToolExecutionRuleRow) -> Result<Self, Self::Error> {
        let action = row
            .action
            .parse::<ToolExecutionRuleAction>()
            .map_err(|_| ToolExecutionRuleRepositoryError::InvalidAction(row.action.clone()))?;

        Ok(Self {
            tool_name: row.tool_name,
            action,
        })
    }
}

fn map_sqlx_error(err: sqlx::Error) -> ToolExecutionRuleRepositoryError {
    ToolExecutionRuleRepositoryError::Unexpected(err.to_string())
}

#[async_trait]
impl ToolExecutionRuleRepository for PostgresToolExecutionRuleRepository {
    async fn find_by_tool_name(
        &self,
        tool_name: &str,
    ) -> Result<Option<ToolExecutionRule>, ToolExecutionRuleRepositoryError> {
        let row = sqlx::query_as::<_, ToolExecutionRuleRow>(
            r#"
            SELECT tool_name, action
            FROM tool_execution_rules
            WHERE tool_name = $1
            "#,
        )
        .bind(tool_name)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(TryInto::try_into).transpose()
    }

    async fn save(&self, rule: ToolExecutionRule) -> Result<(), ToolExecutionRuleRepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO tool_execution_rules (tool_name, action)
            VALUES ($1, $2)
            ON CONFLICT (tool_name)
            DO UPDATE SET
              action = EXCLUDED.action,
              updated_at = NOW()
            "#,
        )
        .bind(rule.tool_name)
        .bind(rule.action.as_str())
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }

    async fn list_all(&self) -> Result<Vec<ToolExecutionRule>, ToolExecutionRuleRepositoryError> {
        let rows = sqlx::query_as::<_, ToolExecutionRuleRow>(
            r#"
        SELECT tool_name, action
        FROM tool_execution_rules
        ORDER BY tool_name
        "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(TryInto::try_into).collect()
    }
}
