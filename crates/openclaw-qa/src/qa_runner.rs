use std::path::Path;

use anyhow::anyhow;
use async_trait::async_trait;
use chrono::Utc;
use db::DBService;
use sqlx::Row;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    test_detector::TestDetector,
    types::{OcQaResult, OcQaRun, OcQaRunStatus, StartQaRequest},
};

#[derive(Debug, Error)]
pub enum QaError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error("Test komutu bulunamadı — workspace dizini: {0}")]
    NoTestCommand(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[async_trait]
pub trait QaRunnerService: Send + Sync {
    /// QA oturumu başlatır; test komutunu çalıştırır.
    async fn start_qa(&self, req: StartQaRequest, workspace_dir: &Path)
    -> Result<OcQaRun, QaError>;

    /// Test sonucunu kaydeder; başarısızsa follow-up prompt metni döner.
    async fn record_result(
        &self,
        qa_run_id: Uuid,
        exit_code: i32,
        output: String,
    ) -> Result<QaOutcome, QaError>;

    /// QA run bilgisini getirir.
    async fn get_qa_run(&self, qa_run_id: Uuid) -> Result<Option<OcQaRun>, QaError>;

    /// Workspace'e ait son QA run'ı getirir.
    async fn latest_qa_run(&self, workspace_id: Uuid) -> Result<Option<OcQaRun>, QaError>;
}

/// Test sonucunun çağırana bildirilen özeti
#[derive(Debug)]
pub enum QaOutcome {
    /// Testler geçti — workspace insan onayına hazır
    Passed,
    /// Testler başarısız, agent'a geri fırlatılacak follow-up prompt
    FailedRetry {
        follow_up_prompt: String,
        retry_count: i64,
    },
    /// Max retry doldu — insan müdahalesi gerekiyor
    Exhausted { last_output: String },
}

pub struct OcQaRunner {
    db: DBService,
}

impl OcQaRunner {
    pub fn new(db: DBService) -> Self {
        Self { db }
    }

    /// Test komutunu `workspace_dir` içinde çalıştırır, sonucu DB'ye kaydeder ve `QaOutcome` döner.
    pub async fn run_qa_and_record(
        &self,
        workspace_id: Uuid,
        execution_process_id: Uuid,
        workspace_dir: &Path,
    ) -> Result<QaOutcome, QaError> {
        let qa_run = self
            .start_qa(
                StartQaRequest {
                    workspace_id,
                    execution_process_id,
                    test_command: None,
                    max_retries: None,
                },
                workspace_dir,
            )
            .await?;

        tracing::info!(
            workspace_id = %workspace_id,
            test_command = %qa_run.test_command,
            "Running test command"
        );

        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&qa_run.test_command)
            .current_dir(workspace_dir)
            .output()
            .await
            .map_err(|e| QaError::Other(anyhow!("Test command spawn failed: {e}")))?;

        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}{stderr}").trim().to_string();

        tracing::info!(
            workspace_id = %workspace_id,
            exit_code,
            "Test command finished"
        );

        self.record_result(qa_run.id, exit_code, combined).await
    }

    /// Hata çıktısından agent için follow-up prompt üretir.
    fn build_follow_up_prompt(test_command: &str, output: &str, attempt: i64) -> String {
        format!(
            "Deneme #{attempt}: `{test_command}` komutu başarısız oldu.\n\n\
             Test çıktısı:\n```\n{output}\n```\n\n\
             Lütfen yukarıdaki hataları düzelt ve tekrar çalışır hale getir.",
        )
    }
}

#[async_trait]
impl QaRunnerService for OcQaRunner {
    async fn start_qa(
        &self,
        req: StartQaRequest,
        workspace_dir: &Path,
    ) -> Result<OcQaRun, QaError> {
        // Test komutunu belirle
        let test_command = if let Some(cmd) = req.test_command {
            cmd
        } else {
            TestDetector::detect(workspace_dir)
                .ok_or_else(|| QaError::NoTestCommand(workspace_dir.display().to_string()))?
        };

        let qa_run_id = Uuid::new_v4();
        let now = Utc::now();
        let max_retries = req.max_retries.unwrap_or(3);

        sqlx::query(
            "INSERT INTO oc_qa_runs (id, workspace_id, execution_process_id, test_command, status, retry_count, max_retries, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, 0, ?, ?, ?)",
        )
        .bind(qa_run_id.to_string())
        .bind(req.workspace_id.to_string())
        .bind(req.execution_process_id.to_string())
        .bind(&test_command)
        .bind(OcQaRunStatus::Running.as_str())
        .bind(max_retries)
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(&self.db.pool)
        .await?;

        tracing::info!(
            workspace_id = %req.workspace_id,
            test_command = %test_command,
            "QA run started"
        );

        Ok(OcQaRun {
            id: qa_run_id,
            workspace_id: req.workspace_id,
            execution_process_id: req.execution_process_id,
            test_command,
            status: OcQaRunStatus::Running,
            retry_count: 0,
            max_retries,
            created_at: now,
            updated_at: now,
        })
    }

    async fn record_result(
        &self,
        qa_run_id: Uuid,
        exit_code: i32,
        output: String,
    ) -> Result<QaOutcome, QaError> {
        let run = self
            .get_qa_run(qa_run_id)
            .await?
            .ok_or_else(|| QaError::Other(anyhow!("QA run bulunamadı: {}", qa_run_id)))?;

        let attempt_number = run.retry_count + 1;
        let passed = exit_code == 0;
        let result_id = Uuid::new_v4();
        let now = Utc::now();

        // Sonucu kaydet
        sqlx::query(
            "INSERT INTO oc_qa_results (id, qa_run_id, attempt_number, exit_code, output, passed, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(result_id.to_string())
        .bind(qa_run_id.to_string())
        .bind(attempt_number)
        .bind(exit_code as i64)
        .bind(&output)
        .bind(if passed { 1i64 } else { 0i64 })
        .bind(now.to_rfc3339())
        .execute(&self.db.pool)
        .await?;

        if passed {
            sqlx::query("UPDATE oc_qa_runs SET status = 'passed', updated_at = ? WHERE id = ?")
                .bind(now.to_rfc3339())
                .bind(qa_run_id.to_string())
                .execute(&self.db.pool)
                .await?;

            tracing::info!(qa_run_id = %qa_run_id, "QA passed");
            return Ok(QaOutcome::Passed);
        }

        // Başarısız — retry sayısını artır
        let new_retry_count = attempt_number;

        if new_retry_count >= run.max_retries {
            sqlx::query(
                "UPDATE oc_qa_runs SET status = 'exhausted', retry_count = ?, updated_at = ? WHERE id = ?",
            )
            .bind(new_retry_count)
            .bind(now.to_rfc3339())
            .bind(qa_run_id.to_string())
            .execute(&self.db.pool)
            .await?;

            tracing::warn!(
                qa_run_id = %qa_run_id,
                retry_count = new_retry_count,
                "QA exhausted max retries"
            );
            return Ok(QaOutcome::Exhausted {
                last_output: output,
            });
        }

        sqlx::query(
            "UPDATE oc_qa_runs SET status = 'failed', retry_count = ?, updated_at = ? WHERE id = ?",
        )
        .bind(new_retry_count)
        .bind(now.to_rfc3339())
        .bind(qa_run_id.to_string())
        .execute(&self.db.pool)
        .await?;

        let follow_up_prompt =
            Self::build_follow_up_prompt(&run.test_command, &output, attempt_number);

        tracing::info!(
            qa_run_id = %qa_run_id,
            retry_count = new_retry_count,
            "QA failed, sending follow-up to agent"
        );

        Ok(QaOutcome::FailedRetry {
            follow_up_prompt,
            retry_count: new_retry_count,
        })
    }

    async fn get_qa_run(&self, qa_run_id: Uuid) -> Result<Option<OcQaRun>, QaError> {
        let row = sqlx::query(
            "SELECT id, workspace_id, execution_process_id, test_command, status,
                    retry_count, max_retries, created_at, updated_at
             FROM oc_qa_runs WHERE id = ?",
        )
        .bind(qa_run_id.to_string())
        .fetch_optional(&self.db.pool)
        .await?;

        let Some(row) = row else { return Ok(None) };
        Ok(Some(Self::row_to_qa_run(row)?))
    }

    async fn latest_qa_run(&self, workspace_id: Uuid) -> Result<Option<OcQaRun>, QaError> {
        let row = sqlx::query(
            "SELECT id, workspace_id, execution_process_id, test_command, status,
                    retry_count, max_retries, created_at, updated_at
             FROM oc_qa_runs WHERE workspace_id = ? ORDER BY created_at DESC LIMIT 1",
        )
        .bind(workspace_id.to_string())
        .fetch_optional(&self.db.pool)
        .await?;

        let Some(row) = row else { return Ok(None) };
        Ok(Some(Self::row_to_qa_run(row)?))
    }
}

impl OcQaRunner {
    fn row_to_qa_run(row: sqlx::sqlite::SqliteRow) -> Result<OcQaRun, QaError> {
        let id: String = row.try_get("id")?;
        let workspace_id: String = row.try_get("workspace_id")?;
        let ep_id: String = row.try_get("execution_process_id")?;
        let status_str: String = row.try_get("status")?;
        let created_at_str: String = row.try_get("created_at")?;
        let updated_at_str: String = row.try_get("updated_at")?;

        Ok(OcQaRun {
            id: id.parse().map_err(|_| sqlx::Error::RowNotFound)?,
            workspace_id: workspace_id.parse().map_err(|_| sqlx::Error::RowNotFound)?,
            execution_process_id: ep_id.parse().map_err(|_| sqlx::Error::RowNotFound)?,
            test_command: row.try_get("test_command")?,
            status: OcQaRunStatus::try_from(status_str.as_str()).unwrap_or(OcQaRunStatus::Pending),
            retry_count: row.try_get("retry_count")?,
            max_retries: row.try_get("max_retries")?,
            created_at: created_at_str.parse().unwrap_or_else(|_| Utc::now()),
            updated_at: updated_at_str.parse().unwrap_or_else(|_| Utc::now()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_follow_up_prompt() {
        let prompt = OcQaRunner::build_follow_up_prompt("cargo test", "error: expected 2 got 3", 1);
        assert!(prompt.contains("cargo test"));
        assert!(prompt.contains("expected 2 got 3"));
        assert!(prompt.contains("Deneme #1"));
    }
}
