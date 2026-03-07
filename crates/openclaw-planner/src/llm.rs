use anyhow::{Context, Result, anyhow};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::types::OcTaskComplexity;

// ── LLM Task (LLM'den gelen ham çıktı) ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmTask {
    pub title: String,
    pub description: String,
    pub complexity: String,
    /// Agent'a verilecek detaylı görev prompt'u.
    pub prompt: Option<String>,
    /// Bağımlı olduğu task başlıkları.
    #[serde(default)]
    pub depends_on: Vec<String>,
}

impl LlmTask {
    pub fn to_complexity(&self) -> OcTaskComplexity {
        OcTaskComplexity::try_from(self.complexity.to_lowercase().as_str())
            .unwrap_or(OcTaskComplexity::Medium)
    }
}

#[derive(Debug, Deserialize)]
struct LlmTaskList {
    tasks: Vec<LlmTask>,
}

// ── Anthropic API types ────────────────────────────────────────────────────

#[derive(Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: Vec<AnthropicMessage<'a>>,
}

#[derive(Serialize)]
struct AnthropicMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

#[derive(Deserialize)]
struct AnthropicContent {
    text: String,
}

// ── AnthropicLlmPlanner ────────────────────────────────────────────────────

pub struct AnthropicLlmPlanner {
    client: Client,
    api_key: String,
}

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
/// Karmaşık planlama için yeterince akıllı, düşük maliyetli model.
const PLANNER_MODEL: &str = "claude-3-5-haiku-20241022";

impl AnthropicLlmPlanner {
    /// `ANTHROPIC_API_KEY` env var'dan okur. Yoksa None döner.
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY").ok()?;
        Some(Self {
            client: Client::new(),
            api_key,
        })
    }

    /// Prompt + codebase context'ten LLM görev listesi üretir.
    pub async fn generate_tasks(
        &self,
        user_prompt: &str,
        codebase_context: Option<&str>,
    ) -> Result<Vec<LlmTask>> {
        let prompt = Self::build_prompt(user_prompt, codebase_context);

        debug!(
            model = PLANNER_MODEL,
            "Sending planning request to Anthropic API"
        );

        let request = AnthropicRequest {
            model: PLANNER_MODEL,
            max_tokens: 4096,
            messages: vec![AnthropicMessage {
                role: "user",
                content: &prompt,
            }],
        };

        let response = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to call Anthropic API")?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Anthropic API error {}: {}", status, body));
        }

        let anthropic_resp: AnthropicResponse = response
            .json()
            .await
            .context("Failed to parse Anthropic API response")?;

        let raw_text = anthropic_resp
            .content
            .into_iter()
            .next()
            .map(|c| c.text)
            .unwrap_or_default();

        info!(chars = raw_text.len(), "Received LLM planning response");

        Self::parse_task_list(&raw_text)
    }

    // ── Private helpers ────────────────────────────────────────────────────

    fn build_prompt(user_prompt: &str, codebase_context: Option<&str>) -> String {
        let ctx_section = match codebase_context {
            Some(ctx) if !ctx.trim().is_empty() => {
                format!("## Kod Tabanı Bilgileri\n\n{}\n\n", ctx)
            }
            _ => String::new(),
        };

        format!(
            r#"Sen deneyimli bir yazılım PM'isin. Aşağıdaki isteği incele ve bağımsız geliştirme görevlerine ayır.

{}## Kullanıcı İsteği

{}

## Görevin

Yukarıdaki isteği net, bağımsız görevlere ayır. Her görev tek bir agent tarafından tamamlanabilir olmalı.

Kurallar:
- Her görev atomik ve net kapsamlı olmalı
- depends_on: Sadece gerçekten önce bitmesi gereken görevleri yaz (title'larla eşleşmeli)
- prompt: Agent'ın tam olarak ne yapacağını açıklayan detaylı İngilizce talimat
- complexity: "low" (< 2 saat), "medium" (2-8 saat), "high" (> 8 saat)
- Mevcut issue'larla çakışan görev oluşturma

Cevabını SADECE geçerli JSON olarak ver, başka açıklama veya metin ekleme:
{{
  "tasks": [
    {{
      "title": "Kısa ve net başlık",
      "description": "Ne yapılacağının özeti",
      "complexity": "low|medium|high",
      "prompt": "Detailed instructions for the coding agent in English. Explain exactly what files to create/modify, what the expected output is, and any specific implementation details.",
      "depends_on": []
    }}
  ]
}}"#,
            ctx_section, user_prompt
        )
    }

    fn parse_task_list(raw: &str) -> Result<Vec<LlmTask>> {
        // LLM bazen ```json ... ``` bloğu içinde döndürür, çıkart
        let json_str = Self::extract_json(raw);

        let parsed: LlmTaskList = serde_json::from_str(json_str).with_context(|| {
            format!(
                "Failed to parse LLM task JSON:\n{}",
                &json_str[..json_str.len().min(500)]
            )
        })?;

        if parsed.tasks.is_empty() {
            warn!("LLM returned empty task list");
        } else {
            info!(count = parsed.tasks.len(), "LLM generated tasks");
        }

        Ok(parsed.tasks)
    }

    /// JSON bloğu varsa içini çıkarır, yoksa olduğu gibi döndürür.
    fn extract_json(text: &str) -> &str {
        // ``` veya ```json bloğu içindeyse
        if let Some(start) = text.find("```json") {
            let after = &text[start + 7..];
            if let Some(end) = after.find("```") {
                return after[..end].trim();
            }
        }
        if let Some(start) = text.find("```") {
            let after = &text[start + 3..];
            if let Some(end) = after.find("```") {
                return after[..end].trim();
            }
        }
        // İlk { ile son } arasını al
        if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}')) {
            return &text[start..=end];
        }
        text.trim()
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_plain() {
        let raw =
            r#"{"tasks":[{"title":"T","description":"D","complexity":"low","depends_on":[]}]}"#;
        let extracted = AnthropicLlmPlanner::extract_json(raw);
        assert!(extracted.starts_with('{'));
    }

    #[test]
    fn test_extract_json_code_block() {
        let raw = "Here is the result:\n```json\n{\"tasks\":[]}\n```\n";
        let extracted = AnthropicLlmPlanner::extract_json(raw);
        assert_eq!(extracted, "{\"tasks\":[]}");
    }

    #[test]
    fn test_parse_task_list() {
        let json = r#"{
            "tasks": [
                {
                    "title": "Auth modülü",
                    "description": "JWT tabanlı kimlik doğrulama",
                    "complexity": "medium",
                    "prompt": "Implement JWT authentication",
                    "depends_on": []
                },
                {
                    "title": "API route'ları",
                    "description": "Login/logout endpoints",
                    "complexity": "low",
                    "prompt": "Create REST endpoints",
                    "depends_on": ["Auth modülü"]
                }
            ]
        }"#;
        let tasks = AnthropicLlmPlanner::parse_task_list(json).unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].title, "Auth modülü");
        assert_eq!(tasks[1].depends_on, vec!["Auth modülü"]);
    }

    #[test]
    fn test_complexity_parsing() {
        let task = LlmTask {
            title: "T".into(),
            description: "D".into(),
            complexity: "HIGH".into(),
            prompt: None,
            depends_on: vec![],
        };
        assert_eq!(task.to_complexity(), OcTaskComplexity::High);
    }

    #[test]
    fn test_build_prompt_includes_context() {
        let prompt =
            AnthropicLlmPlanner::build_prompt("Auth sistemi ekle", Some("## Proje Tipi\nRust"));
        assert!(prompt.contains("Auth sistemi ekle"));
        assert!(prompt.contains("Rust"));
        assert!(prompt.contains("JSON"));
    }
}
