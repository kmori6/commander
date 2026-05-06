use crate::application::error::job_execution_usecase_error::JobExecutionUsecaseError;
use crate::domain::model::app_event::AppEvent;
use crate::domain::model::job::Job;
use crate::domain::model::job_run::JobRun;
use crate::domain::repository::job_repository::JobRepository;
use crate::domain::repository::job_run_repository::JobRunRepository;
use uuid::Uuid;

pub struct JobExecutionOutput {
    pub job: Job,
    pub run: JobRun,
    pub events: Vec<AppEvent>,
}

pub struct JobExecutionUsecase<JR, RR> {
    job_repository: JR,
    run_repository: RR,
}

impl<JR, RR> JobExecutionUsecase<JR, RR>
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

    pub async fn create_run(
        &self,
        job_id: Uuid,
    ) -> Result<JobExecutionOutput, JobExecutionUsecaseError> {
        let job = self
            .job_repository
            .find_by_id(job_id)
            .await?
            .ok_or(JobExecutionUsecaseError::JobNotFound(job_id))?;

        let job = job.start()?;
        let attempt = self.run_repository.next_attempt(job.id).await?;
        let run = JobRun::start(job.id, attempt);

        self.job_repository.update(job.clone()).await?;
        self.run_repository.save(run.clone()).await?;

        Ok(JobExecutionOutput {
            events: vec![AppEvent::JobStarted {
                job_id: job.id,
                status: job.status,
                title: job.title.clone(),
            }],
            job,
            run,
        })
    }

    pub async fn complete_mock(
        &self,
        job_id: Uuid,
        run_id: Uuid,
    ) -> Result<JobExecutionOutput, JobExecutionUsecaseError> {
        let job = self
            .job_repository
            .find_by_id(job_id)
            .await?
            .ok_or(JobExecutionUsecaseError::JobNotFound(job_id))?;
        let run = self
            .run_repository
            .find_by_id(run_id)
            .await?
            .ok_or(JobExecutionUsecaseError::JobRunNotFound(run_id))?;

        if run.job_id != job.id {
            return Err(JobExecutionUsecaseError::JobRunDoesNotBelongToJob { job_id, run_id });
        }

        let job = job.complete()?;
        let run = run.complete()?;

        self.job_repository.update(job.clone()).await?;
        self.run_repository.update(run.clone()).await?;

        Ok(JobExecutionOutput {
            events: vec![AppEvent::JobCompleted {
                job_id: job.id,
                status: job.status,
                title: job.title.clone(),
            }],
            job,
            run,
        })
    }
}
