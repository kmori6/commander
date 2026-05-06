use crate::application::error::job_run_usecase_error::JobRunUsecaseError;
use crate::domain::model::chat_message::ChatMessage;
use crate::domain::model::job_run::JobRun;
use crate::domain::repository::chat_message_repository::ChatMessageRepository;
use crate::domain::repository::job_repository::JobRepository;
use crate::domain::repository::job_run_repository::JobRunRepository;
use uuid::Uuid;

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
}
