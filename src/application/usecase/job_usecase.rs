use crate::application::error::job_usecase_error::JobUsecaseError;
use crate::domain::model::app_event::AppEvent;
use crate::domain::model::job::{Job, JobKind, JobStatus};
use crate::domain::repository::job_repository::JobRepository;
use uuid::Uuid;

pub struct JobUsecaseOutput {
    pub job: Job,
    pub events: Vec<AppEvent>,
}

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
        title: Option<String>,
        objective: impl Into<String>,
        session_id: Option<Uuid>,
        parent_job_id: Option<Uuid>,
    ) -> Result<JobUsecaseOutput, JobUsecaseError> {
        let objective = objective.into();
        let title = Job::title_from_input(title.as_deref(), &objective);

        let job = Job::new(kind, title, objective, session_id, parent_job_id);
        self.repository.save(job.clone()).await?;

        Ok(JobUsecaseOutput {
            events: vec![AppEvent::JobCreated {
                job_id: job.id,
                kind: job.kind,
                status: job.status,
                title: job.title.clone(),
                session_id: job.session_id,
                parent_job_id: job.parent_job_id,
            }],
            job,
        })
    }

    pub async fn find(&self, id: Uuid) -> Result<Option<Job>, JobUsecaseError> {
        Ok(self.repository.find_by_id(id).await?)
    }

    pub async fn list_recent(&self, limit: i64) -> Result<Vec<Job>, JobUsecaseError> {
        Ok(self.repository.list_recent(limit).await?)
    }

    pub async fn start(&self, id: Uuid) -> Result<JobUsecaseOutput, JobUsecaseError> {
        let job = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(JobUsecaseError::JobNotFound(id))?;

        let job = job.start()?;
        self.repository.update(job.clone()).await?;

        Ok(JobUsecaseOutput {
            events: vec![AppEvent::JobStarted {
                job_id: job.id,
                status: job.status,
                title: job.title.clone(),
            }],
            job,
        })
    }

    pub async fn complete(&self, id: Uuid) -> Result<JobUsecaseOutput, JobUsecaseError> {
        let job = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(JobUsecaseError::JobNotFound(id))?;

        let job = job.complete()?;
        self.repository.update(job.clone()).await?;

        Ok(JobUsecaseOutput {
            events: vec![AppEvent::JobCompleted {
                job_id: job.id,
                status: job.status,
                title: job.title.clone(),
            }],
            job,
        })
    }

    pub async fn fail(
        &self,
        id: Uuid,
        reason: impl Into<String>,
    ) -> Result<JobUsecaseOutput, JobUsecaseError> {
        let job = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(JobUsecaseError::JobNotFound(id))?;

        let job = job.fail(reason)?;
        self.repository.update(job.clone()).await?;

        Ok(JobUsecaseOutput {
            events: vec![AppEvent::JobFailed {
                job_id: job.id,
                status: job.status,
                title: job.title.clone(),
                error_message: job.error_message.clone().unwrap_or_default(),
            }],
            job,
        })
    }

    pub async fn cancel(&self, id: Uuid) -> Result<JobUsecaseOutput, JobUsecaseError> {
        let job = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(JobUsecaseError::JobNotFound(id))?;

        let job = job.cancel()?;
        self.repository.update(job.clone()).await?;

        let events = match job.status {
            JobStatus::CancelRequested => vec![AppEvent::JobCancelRequested {
                job_id: job.id,
                status: job.status,
                title: job.title.clone(),
            }],
            JobStatus::Cancelled => vec![AppEvent::JobCancelled {
                job_id: job.id,
                status: job.status,
                title: job.title.clone(),
            }],
            _ => Vec::new(),
        };

        Ok(JobUsecaseOutput { events, job })
    }
}
