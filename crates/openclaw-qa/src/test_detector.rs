use std::path::Path;

/// Repo türüne göre test komutunu otomatik tespit eder.
pub struct TestDetector;

impl TestDetector {
    /// Verilen dizindeki proje dosyalarına göre uygun test komutunu döner.
    pub fn detect(workspace_dir: &Path) -> Option<String> {
        // Rust projesi
        if workspace_dir.join("Cargo.toml").exists() {
            return Some("cargo test --workspace".to_string());
        }

        // Node.js / JavaScript / TypeScript
        if workspace_dir.join("package.json").exists() {
            // package.json içinden "test" script'ini kontrol et
            if let Ok(content) = std::fs::read_to_string(workspace_dir.join("package.json")) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if json["scripts"]["test"].is_string() {
                        // pnpm > yarn > npm tercih sırası
                        if workspace_dir.join("pnpm-lock.yaml").exists() {
                            return Some("pnpm run test".to_string());
                        } else if workspace_dir.join("yarn.lock").exists() {
                            return Some("yarn test".to_string());
                        } else {
                            return Some("npm test".to_string());
                        }
                    }
                }
            }
        }

        // Python
        if workspace_dir.join("pyproject.toml").exists() || workspace_dir.join("setup.py").exists()
        {
            if workspace_dir.join("pytest.ini").exists()
                || workspace_dir.join("pyproject.toml").exists()
            {
                return Some("pytest".to_string());
            }
            return Some("python -m pytest".to_string());
        }

        // Go
        if workspace_dir.join("go.mod").exists() {
            return Some("go test ./...".to_string());
        }

        // Ruby
        if workspace_dir.join("Gemfile").exists() {
            return Some("bundle exec rspec".to_string());
        }

        // PHP
        if workspace_dir.join("composer.json").exists() {
            return Some("./vendor/bin/phpunit".to_string());
        }

        // Java / Maven
        if workspace_dir.join("pom.xml").exists() {
            return Some("mvn test".to_string());
        }

        // Java / Gradle
        if workspace_dir.join("build.gradle").exists()
            || workspace_dir.join("build.gradle.kts").exists()
        {
            return Some("./gradlew test".to_string());
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_detect_rust() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        assert_eq!(
            TestDetector::detect(dir.path()),
            Some("cargo test --workspace".to_string())
        );
    }

    #[test]
    fn test_detect_node_pnpm() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{"scripts": {"test": "vitest"}}"#,
        )
        .unwrap();
        fs::write(dir.path().join("pnpm-lock.yaml"), "").unwrap();
        assert_eq!(
            TestDetector::detect(dir.path()),
            Some("pnpm run test".to_string())
        );
    }

    #[test]
    fn test_detect_go() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("go.mod"), "module test").unwrap();
        assert_eq!(
            TestDetector::detect(dir.path()),
            Some("go test ./...".to_string())
        );
    }

    #[test]
    fn test_detect_unknown() {
        let dir = TempDir::new().unwrap();
        assert_eq!(TestDetector::detect(dir.path()), None);
    }
}
