use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::error::job_run_repository_error::JobRunRepositoryError;
use crate::domain::model::job_run::JobRun;

#[async_trait]
pub trait JobRunRepository: Send + Sync + Clone + 'static {
    async fn save(&self, run: JobRun) -> Result<(), JobRunRepositoryError>;

    async fn update(&self, run: JobRun) -> Result<(), JobRunRepositoryError>;

    async fn find_by_id(&self, id: Uuid) -> Result<Option<JobRun>, JobRunRepositoryError>;

    async fn find_latest_by_job_id(
        &self,
        job_id: Uuid,
    ) -> Result<Option<JobRun>, JobRunRepositoryError>;

    async fn list_by_job_id(&self, job_id: Uuid) -> Result<Vec<JobRun>, JobRunRepositoryError>;

    async fn next_attempt(&self, job_id: Uuid) -> Result<i32, JobRunRepositoryError>;
}
