use crate::application::error::job_run_usecase_error::JobRunUsecaseError;
use crate::domain::model::app_event::AppEvent;
use crate::domain::model::job::Job;
use crate::domain::model::job_run::JobRun;
use crate::domain::repository::job_repository::JobRepository;
use crate::domain::repository::job_run_repository::JobRunRepository;
use uuid::Uuid;

pub struct JobRunUsecaseOutput {
    pub job: Job,
    pub events: Vec<AppEvent>,
}

pub struct JobRunUsecase<JR, RR> {
    job_repository: JR,
    run_repository: RR,
}

impl<JR, RR> JobRunUsecase<JR, RR>
where
    JR: JobRepository,
    RR: JobRunRepository,
{
    pub fn new(job_repository: JR, run_repository: RR) -> Self {
        Self {
            job_repository,
            run_repository,
        }
    }

    pub async fn list(&self, job_id: Uuid) -> Result<Vec<JobRun>, JobRunUsecaseError> {
        self.job_repository
            .find_by_id(job_id)
            .await?
            .ok_or(JobRunUsecaseError::JobNotFound(job_id))?;

        Ok(self.run_repository.list_by_job_id(job_id).await?)
    }

    pub async fn start(&self, job_id: Uuid) -> Result<JobRunUsecaseOutput, JobRunUsecaseError> {
        let job = self
            .job_repository
            .find_by_id(job_id)
            .await?
            .ok_or(JobRunUsecaseError::JobNotFound(job_id))?;

        let job = job.start()?;
        let attempt = self.run_repository.next_attempt(job.id).await?;
        let run = JobRun::start(job.id, attempt);

        self.job_repository.update(job.clone()).await?;
        self.run_repository.save(run).await?;

        Ok(JobRunUsecaseOutput {
            events: vec![AppEvent::JobStarted {
                job_id: job.id,
                status: job.status,
                title: job.title.clone(),
            }],
            job,
        })
    }

    pub async fn complete(&self, job_id: Uuid) -> Result<JobRunUsecaseOutput, JobRunUsecaseError> {
        let job = self
            .job_repository
            .find_by_id(job_id)
            .await?
            .ok_or(JobRunUsecaseError::JobNotFound(job_id))?;

        let job = job.complete()?;
        let run = self.run_repository.find_latest_by_job_id(job.id).await?;

        self.job_repository.update(job.clone()).await?;

        if let Some(run) = run.filter(|run| !run.is_terminal()) {
            self.run_repository.update(run.complete()?).await?;
        }

        Ok(JobRunUsecaseOutput {
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
        job_id: Uuid,
        reason: impl Into<String>,
    ) -> Result<JobRunUsecaseOutput, JobRunUsecaseError> {
        let job = self
            .job_repository
            .find_by_id(job_id)
            .await?
            .ok_or(JobRunUsecaseError::JobNotFound(job_id))?;

        let reason = reason.into();
        let job = job.fail(reason.clone())?;
        let run = self.run_repository.find_latest_by_job_id(job.id).await?;

        self.job_repository.update(job.clone()).await?;

        if let Some(run) = run.filter(|run| !run.is_terminal()) {
            self.run_repository.update(run.fail(reason)?).await?;
        }

        Ok(JobRunUsecaseOutput {
            events: vec![AppEvent::JobFailed {
                job_id: job.id,
                status: job.status,
                title: job.title.clone(),
                error_message: job.error_message.clone().unwrap_or_default(),
            }],
            job,
        })
    }
}
