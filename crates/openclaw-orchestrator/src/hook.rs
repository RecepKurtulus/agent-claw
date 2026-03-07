use async_trait::async_trait;
use db::DBService;
use sqlx::Row;
use thiserror::Error;
use uuid::Uuid;

use crate::orchestrator::{OcOrchestrator, OrchestratorError, OrchestratorService};

#[derive(Debug, Error)]
pub enum HookError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    Orchestrator(#[from] OrchestratorError),
}

/// Agent tamamlandığında container service tarafından çağrılır.
/// workspace_id üzerinden hangi orchestration task'ına ait olduğunu bulur
/// ve orchestrator'ı tetikler.
#[async_trait]
pub trait OcHookService: Send + Sync {
    async fn on_coding_agent_completed(
        &self,
        workspace_id: Uuid,
        success: bool,
    ) -> Result<(), HookError>;
}

pub struct OcOrchestrationHook {
    db: DBService,
}

impl OcOrchestrationHook {
    pub fn new(db: DBService) -> Self {
        Self { db }
    }

    /// workspace_id ile eşleşen aktif oc_task_run_state kaydını bulur.
    async fn find_active_task_run(
        &self,
        workspace_id: Uuid,
    ) -> Result<Option<(Uuid, Uuid)>, sqlx::Error> {
        // (run_id, task_id) döner
        let row = sqlx::query(
            "SELECT run_id, task_id FROM oc_task_run_state
             WHERE workspace_id = ? AND status = 'running'
             LIMIT 1",
        )
        .bind(workspace_id.to_string())
        .fetch_optional(&self.db.pool)
        .await?;

        let Some(row) = row else { return Ok(None) };
        let run_id: String = row.try_get("run_id")?;
        let task_id: String = row.try_get("task_id")?;

        Ok(Some((
            run_id.parse().unwrap_or_else(|_| Uuid::nil()),
            task_id.parse().unwrap_or_else(|_| Uuid::nil()),
        )))
    }
}

#[async_trait]
impl OcHookService for OcOrchestrationHook {
    async fn on_coding_agent_completed(
        &self,
        workspace_id: Uuid,
        success: bool,
    ) -> Result<(), HookError> {
        let Some((run_id, task_id)) = self.find_active_task_run(workspace_id).await? else {
            // Bu workspace OpenClaw tarafından yönetilmiyor — normal akış
            tracing::debug!(
                workspace_id = %workspace_id,
                "No active OC task run for workspace, skipping hook"
            );
            return Ok(());
        };

        let orchestrator = OcOrchestrator::new(self.db.clone());

        if success {
            tracing::info!(
                workspace_id = %workspace_id,
                run_id = %run_id,
                task_id = %task_id,
                "Coding agent completed — notifying orchestrator"
            );
            let unblocked = orchestrator
                .on_task_completed(run_id, task_id, None)
                .await?;

            if !unblocked.is_empty() {
                tracing::info!(
                    run_id = %run_id,
                    unblocked_count = unblocked.len(),
                    "Unblocked tasks: {:?}",
                    unblocked
                );
            }
        } else {
            tracing::warn!(
                workspace_id = %workspace_id,
                run_id = %run_id,
                task_id = %task_id,
                "Coding agent failed — notifying orchestrator"
            );
            orchestrator.on_task_failed(run_id, task_id).await?;
        }

        Ok(())
    }
}

/// Prod dışı / OpenClaw devre dışıyken kullanılan no-op implementasyon.
pub struct NoopOcHook;

#[async_trait]
impl OcHookService for NoopOcHook {
    async fn on_coding_agent_completed(
        &self,
        _workspace_id: Uuid,
        _success: bool,
    ) -> Result<(), HookError> {
        Ok(())
    }
}
