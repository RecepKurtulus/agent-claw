use std::{
    fs,
    path::{Path, PathBuf},
};

use db::DBService;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

// ── Project type detection ─────────────────────────────────────────────────

/// Tespit edilen proje türü.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProjectType {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Go,
    Ruby,
    Java,
    Php,
    Mixed(Vec<String>),
    Unknown,
}

impl ProjectType {
    pub fn label(&self) -> String {
        match self {
            ProjectType::Rust => "Rust".into(),
            ProjectType::TypeScript => "TypeScript".into(),
            ProjectType::JavaScript => "JavaScript".into(),
            ProjectType::Python => "Python".into(),
            ProjectType::Go => "Go".into(),
            ProjectType::Ruby => "Ruby".into(),
            ProjectType::Java => "Java".into(),
            ProjectType::Php => "PHP".into(),
            ProjectType::Mixed(langs) => langs.join(" + "),
            ProjectType::Unknown => "Unknown".into(),
        }
    }
}

/// Taramada bulunan önemli dosya.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyFileInfo {
    /// Repo root'una göre göreli yol.
    pub path: String,
    /// Dosyanın tipi (Manifest, Readme, Config, …).
    pub file_kind: String,
    /// İlk N satır içeriği (fazla token harcamamak için).
    pub snippet: Option<String>,
}

/// Mevcut Kanban task (duplikasyon kontrolü için).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExistingTaskSummary {
    pub title: String,
    pub status: String,
}

/// Planlayıcıya verilecek tam codebase bağlamı.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodebaseContext {
    pub project_type: ProjectType,
    pub key_files: Vec<KeyFileInfo>,
    pub existing_tasks: Vec<ExistingTaskSummary>,
    /// LLM'e verilecek ham metin özeti.
    pub summary: String,
}

// ── Manifest definitions ───────────────────────────────────────────────────

struct ManifestDef {
    filename: &'static str,
    file_kind: &'static str,
}

const MANIFESTS: &[ManifestDef] = &[
    ManifestDef {
        filename: "Cargo.toml",
        file_kind: "Manifest",
    },
    ManifestDef {
        filename: "package.json",
        file_kind: "Manifest",
    },
    ManifestDef {
        filename: "pyproject.toml",
        file_kind: "Manifest",
    },
    ManifestDef {
        filename: "requirements.txt",
        file_kind: "Manifest",
    },
    ManifestDef {
        filename: "go.mod",
        file_kind: "Manifest",
    },
    ManifestDef {
        filename: "Gemfile",
        file_kind: "Manifest",
    },
    ManifestDef {
        filename: "pom.xml",
        file_kind: "Manifest",
    },
    ManifestDef {
        filename: "build.gradle",
        file_kind: "Manifest",
    },
    ManifestDef {
        filename: "composer.json",
        file_kind: "Manifest",
    },
    ManifestDef {
        filename: "README.md",
        file_kind: "Readme",
    },
    ManifestDef {
        filename: "readme.md",
        file_kind: "Readme",
    },
    ManifestDef {
        filename: ".env.example",
        file_kind: "Config",
    },
    ManifestDef {
        filename: "docker-compose.yml",
        file_kind: "Config",
    },
    ManifestDef {
        filename: "Dockerfile",
        file_kind: "Config",
    },
];

// ── CodebaseScanner ────────────────────────────────────────────────────────

pub struct CodebaseScanner {
    db: DBService,
}

impl CodebaseScanner {
    pub fn new(db: DBService) -> Self {
        Self { db }
    }

    /// Repo path'lerini ve proje ID'sini alarak tam bağlam üretir.
    pub async fn scan(&self, project_id: Uuid, repo_paths: &[String]) -> CodebaseContext {
        let key_files = self.scan_key_files(repo_paths);
        let project_type = Self::detect_project_type(&key_files);
        let existing_tasks = self.fetch_existing_tasks(project_id).await;
        let summary = Self::build_summary(&project_type, &key_files, &existing_tasks);

        CodebaseContext {
            project_type,
            key_files,
            existing_tasks,
            summary,
        }
    }

    // ── Private helpers ────────────────────────────────────────────────────

    fn scan_key_files(&self, repo_paths: &[String]) -> Vec<KeyFileInfo> {
        let mut found = Vec::new();

        for repo_path in repo_paths {
            let base = PathBuf::from(repo_path);
            if !base.exists() {
                tracing::debug!(path = %repo_path, "Repo path does not exist, skipping");
                continue;
            }

            // Root level manifest / config dosyaları
            for mdef in MANIFESTS {
                let file_path = base.join(mdef.filename);
                if file_path.exists() {
                    let relative = mdef.filename.to_string();
                    let snippet = Self::read_snippet(&file_path, 25);
                    found.push(KeyFileInfo {
                        path: relative,
                        file_kind: mdef.file_kind.to_string(),
                        snippet,
                    });
                }
            }

            // src/ veya lib/ klasörü var mı?
            for dir in &["src", "lib", "app", "pkg", "cmd"] {
                let dir_path = base.join(dir);
                if dir_path.is_dir() {
                    found.push(KeyFileInfo {
                        path: dir.to_string(),
                        file_kind: "SourceDir".to_string(),
                        snippet: None,
                    });
                }
            }
        }

        found
    }

    fn detect_project_type(key_files: &[KeyFileInfo]) -> ProjectType {
        let mut langs: Vec<String> = Vec::new();

        for file in key_files {
            let path = file.path.as_str();
            let lang = match path {
                "Cargo.toml" => Some("Rust"),
                "package.json" => Some("TypeScript/JavaScript"),
                "pyproject.toml" | "requirements.txt" => Some("Python"),
                "go.mod" => Some("Go"),
                "Gemfile" => Some("Ruby"),
                "pom.xml" | "build.gradle" => Some("Java"),
                "composer.json" => Some("PHP"),
                _ => None,
            };
            if let Some(l) = lang {
                let owned = l.to_string();
                if !langs.contains(&owned) {
                    langs.push(owned);
                }
            }
        }

        match langs.len() {
            0 => ProjectType::Unknown,
            1 => match langs[0].as_str() {
                "Rust" => ProjectType::Rust,
                "TypeScript/JavaScript" => ProjectType::TypeScript,
                "Python" => ProjectType::Python,
                "Go" => ProjectType::Go,
                "Ruby" => ProjectType::Ruby,
                "Java" => ProjectType::Java,
                "PHP" => ProjectType::Php,
                _ => ProjectType::Unknown,
            },
            _ => ProjectType::Mixed(langs),
        }
    }

    async fn fetch_existing_tasks(&self, project_id: Uuid) -> Vec<ExistingTaskSummary> {
        let rows = sqlx::query(
            "SELECT title, status FROM tasks WHERE project_id = ? AND status != 'cancelled'
             ORDER BY created_at DESC LIMIT 30",
        )
        .bind(project_id.to_string())
        .fetch_all(&self.db.pool)
        .await
        .unwrap_or_default();

        rows.into_iter()
            .filter_map(|row| {
                let title: String = row.try_get("title").ok()?;
                let status: String = row.try_get("status").ok()?;
                Some(ExistingTaskSummary { title, status })
            })
            .collect()
    }

    fn build_summary(
        project_type: &ProjectType,
        key_files: &[KeyFileInfo],
        existing_tasks: &[ExistingTaskSummary],
    ) -> String {
        let mut parts = Vec::new();

        // 1) Proje tipi
        parts.push(format!("## Proje Tipi\n{}", project_type.label()));

        // 2) Tespit edilen manifest + config dosyaları
        let manifests: Vec<_> = key_files
            .iter()
            .filter(|f| matches!(f.file_kind.as_str(), "Manifest" | "Config" | "Readme"))
            .collect();

        if !manifests.is_empty() {
            let mut section = "## Anahtar Dosyalar\n".to_string();
            for f in &manifests {
                section.push_str(&format!("- `{}`", f.path));
                if let Some(snip) = &f.snippet {
                    let first_line = snip.lines().next().unwrap_or("").trim();
                    if !first_line.is_empty() {
                        section.push_str(&format!(" → {}", first_line));
                    }
                }
                section.push('\n');
            }
            // Manifest snippet'leri
            for f in manifests
                .iter()
                .filter(|f| f.file_kind == "Manifest")
                .take(3)
            {
                if let Some(snip) = &f.snippet {
                    section.push_str(&format!("\n### {}\n```\n{}\n```\n", f.path, snip));
                }
            }
            parts.push(section);
        }

        // 3) Mevcut task'lar
        if !existing_tasks.is_empty() {
            let mut section =
                "## Mevcut Görevler (Yeni görev oluştururken duplikasyondan kaçın)\n".to_string();
            for task in existing_tasks {
                section.push_str(&format!("- [{}] {}\n", task.status, task.title));
            }
            parts.push(section);
        } else {
            parts.push("## Mevcut Görevler\nHenüz görev yok.".to_string());
        }

        parts.join("\n\n")
    }

    /// Bir dosyanın ilk `max_lines` satırını okur.
    fn read_snippet(path: &Path, max_lines: usize) -> Option<String> {
        let content = fs::read_to_string(path).ok()?;
        let lines: Vec<&str> = content.lines().take(max_lines).collect();
        if lines.is_empty() {
            return None;
        }
        Some(lines.join("\n"))
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_rust() {
        let files = vec![KeyFileInfo {
            path: "Cargo.toml".into(),
            file_kind: "Manifest".into(),
            snippet: None,
        }];
        assert_eq!(
            CodebaseScanner::detect_project_type(&files),
            ProjectType::Rust
        );
    }

    #[test]
    fn test_detect_mixed() {
        let files = vec![
            KeyFileInfo {
                path: "Cargo.toml".into(),
                file_kind: "Manifest".into(),
                snippet: None,
            },
            KeyFileInfo {
                path: "package.json".into(),
                file_kind: "Manifest".into(),
                snippet: None,
            },
        ];
        let pt = CodebaseScanner::detect_project_type(&files);
        assert!(matches!(pt, ProjectType::Mixed(_)));
    }

    #[test]
    fn test_build_summary_no_tasks() {
        let summary = CodebaseScanner::build_summary(
            &ProjectType::Rust,
            &[KeyFileInfo {
                path: "Cargo.toml".into(),
                file_kind: "Manifest".into(),
                snippet: Some("[package]\nname = \"test\"".into()),
            }],
            &[],
        );
        assert!(summary.contains("Rust"));
        assert!(summary.contains("Cargo.toml"));
        assert!(summary.contains("Henüz görev yok"));
    }

    #[test]
    fn test_build_summary_with_tasks() {
        let tasks = vec![
            ExistingTaskSummary {
                title: "Login ekle".into(),
                status: "todo".into(),
            },
            ExistingTaskSummary {
                title: "API yazılacak".into(),
                status: "inprogress".into(),
            },
        ];
        let summary = CodebaseScanner::build_summary(&ProjectType::TypeScript, &[], &tasks);
        assert!(summary.contains("Login ekle"));
        assert!(summary.contains("inprogress"));
    }
}
