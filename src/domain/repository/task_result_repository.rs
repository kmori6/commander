use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::error::task_result_repository_error::TaskResultRepositoryError;
use crate::domain::model::task_result::{TaskResult, TaskResultStatus};

#[async_trait]
pub trait TaskResultRepository: Send + Sync {
    async fn save(
        &self,
        task_id: Uuid,
        status: TaskResultStatus,
        output: String,
    ) -> Result<TaskResult, TaskResultRepositoryError>;

    async fn find_by_task_id(
        &self,
        task_id: Uuid,
    ) -> Result<Option<TaskResult>, TaskResultRepositoryError>;
}
