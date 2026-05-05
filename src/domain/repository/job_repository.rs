use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::error::job_repository_error::JobRepositoryError;
use crate::domain::model::job::Job;

#[async_trait]
pub trait JobRepository: Send + Sync + Clone + 'static {
    async fn save(&self, job: Job) -> Result<(), JobRepositoryError>;

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Job>, JobRepositoryError>;

    async fn list_recent(&self, limit: i64) -> Result<Vec<Job>, JobRepositoryError>;

    async fn update(&self, job: Job) -> Result<(), JobRepositoryError>;
}
