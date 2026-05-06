use crate::application::error::job_execution_usecase_error::JobExecutionUsecaseError;
use crate::application::runtime::agent_runtime::{AgentRuntime, AgentTurnOutcome};
use crate::domain::model::app_event::AppEvent;
use crate::domain::model::chat_session::ChatSession;
use crate::domain::model::job::Job;
use crate::domain::model::job_run::JobRun;
use crate::domain::model::message::Message;
use crate::domain::port::llm_provider::LlmProvider;
use crate::domain::repository::awaiting_tool_approval_repository::AwaitingToolApprovalRepository;
use crate::domain::repository::chat_message_repository::ChatMessageRepository;
use crate::domain::repository::chat_session_repository::ChatSessionRepository;
use crate::domain::repository::job_repository::JobRepository;
use crate::domain::repository::job_run_repository::JobRunRepository;
use crate::domain::repository::token_usage_repository::TokenUsageRepository;
use crate::domain::repository::tool_approval_repository::ToolApprovalRepository;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

pub struct JobExecutionOutput {
    pub job: Job,
    pub run: JobRun,
    pub events: Vec<AppEvent>,
}

pub struct JobExecutionUsecase<L, JR, RR, S, M, T, A, W> {
    agent_runtime: Arc<AgentRuntime<L, S, M, T, A, W>>,
    job_repository: JR,
    run_repository: RR,
    chat_session_repository: S,
    chat_message_repository: M,
}

impl<L, JR, RR, S, M, T, A, W> JobExecutionUsecase<L, JR, RR, S, M, T, A, W>
where
    L: LlmProvider,
    JR: JobRepository,
    RR: JobRunRepository,
    S: ChatSessionRepository,
    M: ChatMessageRepository,
    T: TokenUsageRepository,
    A: ToolApprovalRepository,
    W: AwaitingToolApprovalRepository,
{
    pub fn new(
        agent_runtime: Arc<AgentRuntime<L, S, M, T, A, W>>,
        job_repository: JR,
        run_repository: RR,
        chat_session_repository: S,
        chat_message_repository: M,
    ) -> Self {
        Self {
            agent_runtime,
            job_repository,
            run_repository,
            chat_session_repository,
            chat_message_repository,
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

    pub async fn execute_run(
        &self,
        job_id: Uuid,
        run_id: Uuid,
        tx: mpsc::Sender<AppEvent>,
    ) -> Result<JobExecutionOutput, JobExecutionUsecaseError> {
        let job = self.load_job(job_id).await?;
        let run = self.load_run(job_id, run_id).await?;

        let session = self.ensure_session(&job).await?;
        let next_status = session.start_turn()?;
        self.chat_session_repository
            .update_status(session.id, next_status)
            .await?;

        let message = Message::input_text(job.objective.clone())?;
        let saved_message = self
            .chat_message_repository
            .append(session.id, Some(run.id), message)
            .await?;

        let output = match self
            .agent_runtime
            .run_turn(session.id, saved_message, tx)
            .await
        {
            Ok(output) => output,
            Err(err) => {
                return self.fail_run(job_id, run_id, err.to_string()).await;
            }
        };

        match output.outcome {
            AgentTurnOutcome::Completed => self.complete_run(job_id, run_id).await,
            AgentTurnOutcome::AwaitingApproval => Ok(JobExecutionOutput {
                job,
                run,
                events: vec![],
            }),
        }
    }

    async fn load_job(&self, job_id: Uuid) -> Result<Job, JobExecutionUsecaseError> {
        self.job_repository
            .find_by_id(job_id)
            .await?
            .ok_or(JobExecutionUsecaseError::JobNotFound(job_id))
    }

    async fn load_run(
        &self,
        job_id: Uuid,
        run_id: Uuid,
    ) -> Result<JobRun, JobExecutionUsecaseError> {
        let run = self
            .run_repository
            .find_by_id(run_id)
            .await?
            .ok_or(JobExecutionUsecaseError::JobRunNotFound(run_id))?;

        if run.job_id != job_id {
            return Err(JobExecutionUsecaseError::JobRunDoesNotBelongToJob { job_id, run_id });
        }

        Ok(run)
    }

    async fn complete_run(
        &self,
        job_id: Uuid,
        run_id: Uuid,
    ) -> Result<JobExecutionOutput, JobExecutionUsecaseError> {
        let job = self.load_job(job_id).await?;
        let run = self.load_run(job_id, run_id).await?;

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

    async fn fail_run(
        &self,
        job_id: Uuid,
        run_id: Uuid,
        reason: impl Into<String>,
    ) -> Result<JobExecutionOutput, JobExecutionUsecaseError> {
        let reason = reason.into();
        let job = self.load_job(job_id).await?;
        let run = self.load_run(job_id, run_id).await?;

        let job = job.fail(reason.clone())?;
        let run = run.fail(reason.clone())?;

        self.job_repository.update(job.clone()).await?;
        self.run_repository.update(run.clone()).await?;

        Ok(JobExecutionOutput {
            events: vec![AppEvent::JobFailed {
                job_id: job.id,
                status: job.status,
                title: job.title.clone(),
                error_message: reason,
            }],
            job,
            run,
        })
    }

    async fn ensure_session(&self, job: &Job) -> Result<ChatSession, JobExecutionUsecaseError> {
        if let Some(session_id) = job.session_id {
            return self
                .chat_session_repository
                .find_by_id(session_id)
                .await?
                .ok_or(JobExecutionUsecaseError::ChatSessionNotFound(session_id));
        }

        self.chat_session_repository
            .create()
            .await
            .map_err(Into::into)
    }
}
