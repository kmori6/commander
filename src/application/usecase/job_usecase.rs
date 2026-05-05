use uuid::Uuid;

use crate::application::error::job_usecase_error::JobUsecaseError;
use crate::domain::model::job::{Job, JobKind};
use crate::domain::repository::job_repository::JobRepository;

pub struct JobUsecase<R> {
    repository: R,
}

impl<R> JobUsecase<R>
where
    R: JobRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn create(
        &self,
        kind: JobKind,
        title: impl Into<String>,
        session_id: Option<Uuid>,
    ) -> Result<Job, JobUsecaseError> {
        let job = Job::new(kind, title, session_id);
        self.repository.save(job.clone()).await?;
        Ok(job)
    }

    pub async fn find(&self, id: Uuid) -> Result<Option<Job>, JobUsecaseError> {
        Ok(self.repository.find_by_id(id).await?)
    }

    pub async fn list_recent(&self, limit: i64) -> Result<Vec<Job>, JobUsecaseError> {
        Ok(self.repository.list_recent(limit).await?)
    }

    pub async fn start(&self, id: Uuid) -> Result<Job, JobUsecaseError> {
        let job = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(JobUsecaseError::JobNotFound(id))?;

        let job = job.start();
        self.repository.update(job.clone()).await?;
        Ok(job)
    }

    pub async fn complete(&self, id: Uuid) -> Result<Job, JobUsecaseError> {
        let job = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(JobUsecaseError::JobNotFound(id))?;

        let job = job.complete();
        self.repository.update(job.clone()).await?;
        Ok(job)
    }

    pub async fn fail(&self, id: Uuid, reason: impl Into<String>) -> Result<Job, JobUsecaseError> {
        let job = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(JobUsecaseError::JobNotFound(id))?;

        let job = job.fail(reason);
        self.repository.update(job.clone()).await?;
        Ok(job)
    }

    pub async fn request_cancel(&self, id: Uuid) -> Result<Job, JobUsecaseError> {
        let job = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(JobUsecaseError::JobNotFound(id))?;

        let job = job.request_cancel();
        self.repository.update(job.clone()).await?;
        Ok(job)
    }

    pub async fn cancel(&self, id: Uuid) -> Result<Job, JobUsecaseError> {
        let job = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(JobUsecaseError::JobNotFound(id))?;

        let job = job.cancel();
        self.repository.update(job.clone()).await?;
        Ok(job)
    }
}
