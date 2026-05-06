use crate::application::error::job_usecase_error::JobUsecaseError;
use crate::domain::model::app_event::AppEvent;
use crate::domain::model::job::{Job, JobKind, JobStatus};
use crate::domain::model::job_run::JobRun;
use crate::domain::repository::job_repository::JobRepository;
use crate::domain::repository::job_run_repository::JobRunRepository;
use uuid::Uuid;

pub struct JobUsecaseOutput {
    pub job: Job,
    pub events: Vec<AppEvent>,
}

pub struct JobUsecase<R, RR> {
    repository: R,
    run_repository: RR,
}

impl<R, RR> JobUsecase<R, RR>
where
    R: JobRepository,
    RR: JobRunRepository,
{
    pub fn new(repository: R, run_repository: RR) -> Self {
        Self {
            repository,
            run_repository,
        }
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

    pub async fn list_runs(&self, id: Uuid) -> Result<Vec<JobRun>, JobUsecaseError> {
        self.repository
            .find_by_id(id)
            .await?
            .ok_or(JobUsecaseError::JobNotFound(id))?;

        Ok(self.run_repository.list_by_job_id(id).await?)
    }

    pub async fn start(&self, id: Uuid) -> Result<JobUsecaseOutput, JobUsecaseError> {
        let job = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(JobUsecaseError::JobNotFound(id))?;

        let job = job.start()?;
        let attempt = self.run_repository.next_attempt(job.id).await?;
        let run = JobRun::start(job.id, attempt);

        self.repository.update(job.clone()).await?;
        self.run_repository.save(run).await?;

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
        let run = self.run_repository.find_latest_by_job_id(job.id).await?;

        self.repository.update(job.clone()).await?;

        if let Some(run) = run.filter(|run| !run.is_terminal()) {
            self.run_repository.update(run.complete()?).await?;
        }

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

        let reason = reason.into();
        let job = job.fail(reason.clone())?;
        let run = self.run_repository.find_latest_by_job_id(job.id).await?;

        self.repository.update(job.clone()).await?;

        if let Some(run) = run.filter(|run| !run.is_terminal()) {
            self.run_repository.update(run.fail(reason)?).await?;
        }

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
