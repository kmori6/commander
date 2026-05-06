use crate::application::error::approval_usecase_error::ApprovalUsecaseError;
use crate::domain::model::awaiting_tool_approval::AwaitingToolApproval;
use crate::domain::repository::awaiting_tool_approval_repository::AwaitingToolApprovalRepository;

pub struct ApprovalUsecase<W> {
    awaiting_tool_approval_repository: W,
}

impl<W> ApprovalUsecase<W>
where
    W: AwaitingToolApprovalRepository,
{
    pub fn new(awaiting_tool_approval_repository: W) -> Self {
        Self {
            awaiting_tool_approval_repository,
        }
    }

    pub async fn list_awaiting(&self) -> Result<Vec<AwaitingToolApproval>, ApprovalUsecaseError> {
        self.awaiting_tool_approval_repository
            .list_all()
            .await
            .map_err(Into::into)
    }
}
