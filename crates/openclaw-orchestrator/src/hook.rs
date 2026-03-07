use std::path::Path;

use async_trait::async_trait;
use db::DBService;
use openclaw_qa::{OcQaRunner, QaOutcome, QaRunnerService, StartQaRequest};
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
    #[error(transparent)]
    Qa(#[from] openclaw_qa::QaError),
}

/// `trigger_oc_hook` tarafından döndürülen, container'ın ne yapması gerektiğini belirten sonuç.
#[derive(Debug)]
pub enum OcHookResult {
    /// Workspace, OpenClaw tarafından yönetilmiyor — normal akış.
    NotManaged,
    /// Agent başarısız oldu; orchestrator bilgilendirildi.
    AgentFailed,
    /// QA geçti; `unblocked` içindeki task'lar artık çalışabilir.
    QaPassed { unblocked: Vec<Uuid> },
    /// QA başarısız; agent'a gönderilecek follow-up prompt.
    QaFailed { follow_up_prompt: String },
    /// QA max retry limitini aştı; insan müdahalesi gerekiyor.
    QaExhausted { last_output: String },
    /// Workspace dizininde test komutu bulunamadı; orchestrator bilgilendirildi.
    QaSkipped { unblocked: Vec<Uuid> },
}

/// Agent tamamlandığında container service tarafından çağrılır.
#[async_trait]
pub trait OcHookService: Send + Sync {
    async fn on_coding_agent_completed(
        &self,
        workspace_id: Uuid,
        execution_process_id: Uuid,
        workspace_dir: &Path,
        success: bool,
    ) -> Result<OcHookResult, HookError>;
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

    /// Bir EP'e ait CodingAgentTurn'ün son özetini getirir.
    async fn fetch_execution_summary(&self, execution_process_id: Uuid) -> Option<String> {
        let row = sqlx::query(
            "SELECT summary FROM coding_agent_turns
             WHERE execution_process_id = ? AND summary IS NOT NULL
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(execution_process_id.to_string())
        .fetch_optional(&self.db.pool)
        .await
        .ok()??;

        row.try_get::<Option<String>, _>("summary").ok()?
    }
}

#[async_trait]
impl OcHookService for OcOrchestrationHook {
    async fn on_coding_agent_completed(
        &self,
        workspace_id: Uuid,
        execution_process_id: Uuid,
        workspace_dir: &Path,
        success: bool,
    ) -> Result<OcHookResult, HookError> {
        let Some((run_id, task_id)) = self.find_active_task_run(workspace_id).await? else {
            tracing::debug!(
                workspace_id = %workspace_id,
                "No active OC task run for workspace, skipping hook"
            );
            return Ok(OcHookResult::NotManaged);
        };

        let orchestrator = OcOrchestrator::new(self.db.clone());

        if !success {
            tracing::warn!(
                workspace_id = %workspace_id,
                run_id = %run_id,
                task_id = %task_id,
                "Coding agent failed — notifying orchestrator"
            );
            orchestrator.on_task_failed(run_id, task_id).await?;
            return Ok(OcHookResult::AgentFailed);
        }

        tracing::info!(
            workspace_id = %workspace_id,
            run_id = %run_id,
            task_id = %task_id,
            "Coding agent completed — running QA"
        );

        // QA'yı çalıştır
        let qa_runner = OcQaRunner::new(self.db.clone());
        let qa_result = qa_runner
            .run_qa_and_record(workspace_id, execution_process_id, workspace_dir)
            .await;

        match qa_result {
            Err(openclaw_qa::QaError::NoTestCommand(_)) => {
                tracing::info!(
                    workspace_id = %workspace_id,
                    "No test command found — skipping QA, marking task completed"
                );
                let summary = self.fetch_execution_summary(execution_process_id).await;
                let unblocked = orchestrator
                    .on_task_completed(run_id, task_id, summary)
                    .await?;
                return Ok(OcHookResult::QaSkipped { unblocked });
            }
            Err(e) => return Err(HookError::Qa(e)),
            Ok(outcome) => match outcome {
                QaOutcome::Passed => {
                    tracing::info!(workspace_id = %workspace_id, "QA passed");
                    let summary = self.fetch_execution_summary(execution_process_id).await;
                    if let Some(ref s) = summary {
                        tracing::debug!(
                            workspace_id = %workspace_id,
                            summary_len = s.len(),
                            "Attaching execution summary to task state"
                        );
                    }
                    let unblocked = orchestrator
                        .on_task_completed(run_id, task_id, summary)
                        .await?;
                    Ok(OcHookResult::QaPassed { unblocked })
                }
                QaOutcome::FailedRetry {
                    follow_up_prompt, ..
                } => {
                    tracing::info!(
                        workspace_id = %workspace_id,
                        "QA failed — sending follow-up to agent"
                    );
                    Ok(OcHookResult::QaFailed { follow_up_prompt })
                }
                QaOutcome::Exhausted { last_output } => {
                    tracing::warn!(
                        workspace_id = %workspace_id,
                        "QA exhausted retries — notifying orchestrator of failure"
                    );
                    orchestrator.on_task_failed(run_id, task_id).await?;
                    Ok(OcHookResult::QaExhausted { last_output })
                }
            },
        }
    }
}

/// Prod dışı / OpenClaw devre dışıyken kullanılan no-op implementasyon.
pub struct NoopOcHook;

#[async_trait]
impl OcHookService for NoopOcHook {
    async fn on_coding_agent_completed(
        &self,
        _workspace_id: Uuid,
        _execution_process_id: Uuid,
        _workspace_dir: &Path,
        _success: bool,
    ) -> Result<OcHookResult, HookError> {
        Ok(OcHookResult::NotManaged)
    }
}
