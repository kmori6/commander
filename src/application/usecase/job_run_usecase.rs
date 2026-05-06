use crate::application::error::job_run_usecase_error::JobRunUsecaseError;
use crate::domain::model::app_event::AppEvent;
use crate::domain::model::chat_message::ChatMessage;
use crate::domain::model::job::Job;
use crate::domain::model::job_run::JobRun;
use crate::domain::repository::chat_message_repository::ChatMessageRepository;
use crate::domain::repository::job_repository::JobRepository;
use crate::domain::repository::job_run_repository::JobRunRepository;
use uuid::Uuid;

pub struct JobRunUsecaseOutput {
    pub job: Job,
    pub run: Option<JobRun>,
    pub events: Vec<AppEvent>,
}

pub struct JobRunUsecase<JR, RR, MR> {
    job_repository: JR,
    run_repository: RR,
    message_repository: MR,
}

impl<JR, RR, MR> JobRunUsecase<JR, RR, MR>
where
    JR: JobRepository,
    RR: JobRunRepository,
    MR: ChatMessageRepository,
{
    pub fn new(job_repository: JR, run_repository: RR, message_repository: MR) -> Self {
        Self {
            job_repository,
            run_repository,
            message_repository,
        }
    }

    pub async fn list(&self, job_id: Uuid) -> Result<Vec<JobRun>, JobRunUsecaseError> {
        self.job_repository
            .find_by_id(job_id)
            .await?
            .ok_or(JobRunUsecaseError::JobNotFound(job_id))?;

        Ok(self.run_repository.list_by_job_id(job_id).await?)
    }

    pub async fn messages(
        &self,
        job_id: Uuid,
        run_id: Uuid,
    ) -> Result<Vec<ChatMessage>, JobRunUsecaseError> {
        self.job_repository
            .find_by_id(job_id)
            .await?
            .ok_or(JobRunUsecaseError::JobNotFound(job_id))?;

        let run = self
            .run_repository
            .find_by_id(run_id)
            .await?
            .ok_or(JobRunUsecaseError::JobRunNotFound(run_id))?;

        if run.job_id != job_id {
            return Err(JobRunUsecaseError::JobRunDoesNotBelongToJob { job_id, run_id });
        }

        Ok(self.message_repository.list_for_job_run(run_id).await?)
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
        self.run_repository.save(run.clone()).await?;

        let events = vec![AppEvent::JobStarted {
            job_id: job.id,
            status: job.status,
            title: job.title.clone(),
        }];

        Ok(JobRunUsecaseOutput {
            job,
            run: Some(run),
            events,
        })
    }

    pub async fn complete(&self, job_id: Uuid) -> Result<JobRunUsecaseOutput, JobRunUsecaseError> {
        let job = self
            .job_repository
            .find_by_id(job_id)
            .await?
            .ok_or(JobRunUsecaseError::JobNotFound(job_id))?;

        let job = job.complete()?;
        let latest_run = self.run_repository.find_latest_by_job_id(job.id).await?;

        self.job_repository.update(job.clone()).await?;

        let completed_run = match latest_run {
            Some(run) if run.is_terminal() => Some(run),
            Some(run) => {
                let run = run.complete()?;
                self.run_repository.update(run.clone()).await?;
                Some(run)
            }
            None => None,
        };

        let events = vec![AppEvent::JobCompleted {
            job_id: job.id,
            status: job.status,
            title: job.title.clone(),
        }];

        Ok(JobRunUsecaseOutput {
            job,
            run: completed_run,
            events,
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
        let latest_run = self.run_repository.find_latest_by_job_id(job.id).await?;

        self.job_repository.update(job.clone()).await?;

        let failed_run = match latest_run {
            Some(run) if run.is_terminal() => Some(run),
            Some(run) => {
                let run = run.fail(reason)?;
                self.run_repository.update(run.clone()).await?;
                Some(run)
            }
            None => None,
        };

        let events = vec![AppEvent::JobFailed {
            job_id: job.id,
            status: job.status,
            title: job.title.clone(),
            error_message: job.error_message.clone().unwrap_or_default(),
        }];

        Ok(JobRunUsecaseOutput {
            job,
            run: failed_run,
            events,
        })
    }
}
