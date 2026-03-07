use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OcQaRun {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub execution_process_id: Uuid,
    pub test_command: String,
    pub status: OcQaRunStatus,
    pub retry_count: i64,
    pub max_retries: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export)]
pub enum OcQaRunStatus {
    Pending,
    Running,
    Passed,
    /// Test başarısız — agent'a geri fırlatıldı, retry bekliyor
    Failed,
    /// Max retry sayısına ulaşıldı, insan müdahalesi gerekiyor
    Exhausted,
}

impl OcQaRunStatus {
    pub fn as_str(&self) -> &str {
        match self {
            OcQaRunStatus::Pending => "pending",
            OcQaRunStatus::Running => "running",
            OcQaRunStatus::Passed => "passed",
            OcQaRunStatus::Failed => "failed",
            OcQaRunStatus::Exhausted => "exhausted",
        }
    }
}

impl TryFrom<&str> for OcQaRunStatus {
    type Error = anyhow::Error;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "pending" => Ok(OcQaRunStatus::Pending),
            "running" => Ok(OcQaRunStatus::Running),
            "passed" => Ok(OcQaRunStatus::Passed),
            "failed" => Ok(OcQaRunStatus::Failed),
            "exhausted" => Ok(OcQaRunStatus::Exhausted),
            other => Err(anyhow::anyhow!("Unknown OcQaRunStatus: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OcQaResult {
    pub id: Uuid,
    pub qa_run_id: Uuid,
    pub attempt_number: i64,
    pub exit_code: Option<i64>,
    pub output: String,
    pub passed: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StartQaRequest {
    pub workspace_id: Uuid,
    pub execution_process_id: Uuid,
    /// Belirtilmezse test_detector otomatik bulur
    pub test_command: Option<String>,
    pub max_retries: Option<i64>,
}
