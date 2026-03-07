use std::collections::HashSet;

use crate::types::{OcDuplicationWarning, OcPlanTask};

// ── Stop words ─────────────────────────────────────────────────────────────

/// Türkçe + İngilizce sık kullanılan kelimeler (benzerlik hesabında göz ardı edilir).
const STOP_WORDS: &[&str] = &[
    // Türkçe
    "ve", "bir", "bu", "ile", "için", "de", "da", "mi", "mu", "mü", "mı", "ne", "ki", "ya", "ama",
    "veya", "ancak", "olan", "olarak", "gibi", "kadar", "sonra", "önce", "her", "tüm", "bütün",
    "bazı", "çok", // İngilizce
    "the", "and", "for", "add", "with", "this", "that", "from", "into", "are", "have", "has",
    "been", "will", "should", "can", "may", "not", "but", "its", "via", "per",
];

// ── DuplicationChecker ─────────────────────────────────────────────────────

pub struct ExistingTask {
    pub title: String,
    pub status: String,
}

pub struct DuplicationChecker;

impl DuplicationChecker {
    /// Yeni task'ları mevcut açık task'larla karşılaştırır.
    /// `threshold`: 0.0–1.0 arası Jaccard skoru eşiği (önerilen: 0.30).
    pub fn check(
        new_tasks: &[OcPlanTask],
        existing: &[ExistingTask],
        threshold: f32,
    ) -> Vec<OcDuplicationWarning> {
        let mut warnings = Vec::new();

        for new_task in new_tasks {
            // Yeni task için birleşik metin (başlık + açıklama)
            let new_text = format!("{} {}", new_task.title, new_task.description);
            let new_tokens = Self::tokenize(&new_text);

            if new_tokens.is_empty() {
                continue;
            }

            // Her mevcut task ile karşılaştır
            for existing_task in existing {
                let existing_tokens = Self::tokenize(&existing_task.title);
                if existing_tokens.is_empty() {
                    continue;
                }

                let score = Self::jaccard(&new_tokens, &existing_tokens);
                if score >= threshold {
                    warnings.push(OcDuplicationWarning {
                        new_task_title: new_task.title.clone(),
                        similar_task_title: existing_task.title.clone(),
                        similarity_score: score,
                        existing_status: existing_task.status.clone(),
                    });
                }
            }
        }

        // En yüksek skor önce
        warnings.sort_by(|a, b| b.similarity_score.partial_cmp(&a.similarity_score).unwrap());

        warnings
    }

    // ── Private helpers ────────────────────────────────────────────────────

    /// Jaccard benzerliği: |A ∩ B| / |A ∪ B|
    fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f32 {
        if a.is_empty() && b.is_empty() {
            return 0.0;
        }
        let intersection = a.intersection(b).count();
        let union = a.union(b).count();
        intersection as f32 / union as f32
    }

    /// Metni küçük harfe çevirir, stop word'leri filtreler, token set'i döner.
    fn tokenize(text: &str) -> HashSet<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphabetic())
            .filter(|w| w.len() > 2)
            .filter(|w| !STOP_WORDS.contains(w))
            .map(String::from)
            .collect()
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use uuid::Uuid;

    use super::*;
    use crate::types::OcTaskComplexity;

    fn make_oc_task(title: &str, description: &str) -> OcPlanTask {
        OcPlanTask {
            id: Uuid::new_v4(),
            plan_id: Uuid::new_v4(),
            issue_id: None,
            title: title.to_string(),
            description: description.to_string(),
            prompt: None,
            estimated_complexity: OcTaskComplexity::Low,
            depends_on: vec![],
            order_index: 0,
            created_at: Utc::now(),
        }
    }

    fn make_existing(title: &str, status: &str) -> ExistingTask {
        ExistingTask {
            title: title.to_string(),
            status: status.to_string(),
        }
    }

    #[test]
    fn test_exact_duplicate_detected() {
        let new = vec![make_oc_task("Auth sistemi ekle", "JWT tabanlı")];
        let existing = vec![make_existing("Auth sistemi ekle", "todo")];

        let warnings = DuplicationChecker::check(&new, &existing, 0.3);
        assert!(!warnings.is_empty());
        assert_eq!(warnings[0].similar_task_title, "Auth sistemi ekle");
        assert!(warnings[0].similarity_score > 0.5);
    }

    #[test]
    fn test_similar_detected_above_threshold() {
        let new = vec![make_oc_task("Kullanıcı kimlik doğrulama", "login logout")];
        let existing = vec![make_existing("Kullanıcı giriş sistemi", "inprogress")];

        // "kullanıcı" overlaps
        let warnings = DuplicationChecker::check(&new, &existing, 0.1);
        assert!(!warnings.is_empty());
    }

    #[test]
    fn test_unrelated_no_warning() {
        let new = vec![make_oc_task("Ödeme sistemi", "stripe entegrasyon")];
        let existing = vec![make_existing("CSS düzenlemesi", "todo")];

        let warnings = DuplicationChecker::check(&new, &existing, 0.3);
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_empty_existing_no_warnings() {
        let new = vec![make_oc_task("API yazılacak", "endpoint ekle")];
        let warnings = DuplicationChecker::check(&new, &[], 0.3);
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_warnings_sorted_by_score_desc() {
        let new = vec![make_oc_task("Auth ve login sistemi", "kimlik doğrulama")];
        let existing = vec![
            make_existing("Login sayfası", "todo"),
            make_existing("Auth sistemi ve kimlik doğrulama", "todo"),
        ];

        let warnings = DuplicationChecker::check(&new, &existing, 0.1);
        if warnings.len() >= 2 {
            assert!(warnings[0].similarity_score >= warnings[1].similarity_score);
        }
    }

    #[test]
    fn test_stop_words_filtered() {
        // "ve" ve "bir" stop word — gerçek örtüşme yok
        let new = vec![make_oc_task("ve bir şey", "test")];
        let existing = vec![make_existing("ve bir başka şey", "todo")];
        // "şey" common, "başka" different — çok düşük skor beklenir
        let warnings = DuplicationChecker::check(&new, &existing, 0.5);
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_jaccard_calculation() {
        let a: std::collections::HashSet<String> = ["auth", "login", "token"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let b: std::collections::HashSet<String> = ["auth", "login", "session"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let score = DuplicationChecker::jaccard(&a, &b);
        // intersection=2, union=4 → 0.5
        assert!((score - 0.5).abs() < 0.01);
    }
}
