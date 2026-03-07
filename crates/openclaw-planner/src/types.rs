use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OcPlan {
    pub id: Uuid,
    pub project_id: Uuid,
    pub prompt: String,
    pub status: OcPlanStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export)]
pub enum OcPlanStatus {
    Pending,
    Analyzing,
    Ready,
    Running,
    Completed,
    Failed,
}

impl OcPlanStatus {
    pub fn as_str(&self) -> &str {
        match self {
            OcPlanStatus::Pending => "pending",
            OcPlanStatus::Analyzing => "analyzing",
            OcPlanStatus::Ready => "ready",
            OcPlanStatus::Running => "running",
            OcPlanStatus::Completed => "completed",
            OcPlanStatus::Failed => "failed",
        }
    }
}

impl TryFrom<&str> for OcPlanStatus {
    type Error = anyhow::Error;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "pending" => Ok(OcPlanStatus::Pending),
            "analyzing" => Ok(OcPlanStatus::Analyzing),
            "ready" => Ok(OcPlanStatus::Ready),
            "running" => Ok(OcPlanStatus::Running),
            "completed" => Ok(OcPlanStatus::Completed),
            "failed" => Ok(OcPlanStatus::Failed),
            other => Err(anyhow::anyhow!("Unknown OcPlanStatus: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OcPlanTask {
    pub id: Uuid,
    pub plan_id: Uuid,
    pub issue_id: Option<Uuid>,
    pub title: String,
    pub description: String,
    pub estimated_complexity: OcTaskComplexity,
    pub order_index: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export)]
pub enum OcTaskComplexity {
    Low,
    Medium,
    High,
}

impl OcTaskComplexity {
    pub fn as_str(&self) -> &str {
        match self {
            OcTaskComplexity::Low => "low",
            OcTaskComplexity::Medium => "medium",
            OcTaskComplexity::High => "high",
        }
    }
}

impl TryFrom<&str> for OcTaskComplexity {
    type Error = anyhow::Error;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "low" => Ok(OcTaskComplexity::Low),
            "medium" => Ok(OcTaskComplexity::Medium),
            "high" => Ok(OcTaskComplexity::High),
            other => Err(anyhow::anyhow!("Unknown OcTaskComplexity: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreateOcPlanRequest {
    pub project_id: Uuid,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreateOcPlanResponse {
    pub plan: OcPlan,
    pub tasks: Vec<OcPlanTask>,
}
