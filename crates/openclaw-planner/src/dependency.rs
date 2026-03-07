use std::collections::{HashMap, VecDeque};

use thiserror::Error;
use uuid::Uuid;

use crate::types::OcPlanTask;

// ── Error ──────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum DependencyError {
    #[error("Döngüsel bağımlılık tespit edildi")]
    CycleDetected,
}

// ── Resolved output ────────────────────────────────────────────────────────

/// Başarılı bağımlılık çözümlemesinin sonucu.
#[derive(Debug)]
pub struct ResolvedDeps {
    /// (task_id, depends_on_task_id) çiftleri — DB'ye yazılacak kenarlar.
    pub edges: Vec<(Uuid, Uuid)>,
    /// Topological sıralamada task ID'leri (0-indeks = en bağımsız).
    pub ordered_task_ids: Vec<Uuid>,
}

// ── PlanDependencyResolver ─────────────────────────────────────────────────

pub struct PlanDependencyResolver;

impl PlanDependencyResolver {
    /// `OcPlanTask.depends_on` (title listesi) → gerçek UUID bağımlılıklarına çevirir.
    /// Döngü varsa `DependencyError::CycleDetected` döner.
    /// Bilinmeyen title'lar uyarı verilerek atlanır (sert hata değil).
    pub fn resolve(tasks: &[OcPlanTask]) -> Result<ResolvedDeps, DependencyError> {
        // 1. Title → ID haritası
        let title_to_id: HashMap<&str, Uuid> =
            tasks.iter().map(|t| (t.title.as_str(), t.id)).collect();

        // 2. Title'ları UUID'lere çevir
        let mut edges: Vec<(Uuid, Uuid)> = Vec::new();
        for task in tasks {
            for dep_title in &task.depends_on {
                match title_to_id.get(dep_title.as_str()) {
                    Some(&dep_id) if dep_id != task.id => {
                        edges.push((task.id, dep_id));
                    }
                    Some(_) => {
                        tracing::warn!(task = %task.title, "Task kendine bağımlı (atlandı)");
                    }
                    None => {
                        tracing::warn!(
                            task = %task.title,
                            dep = %dep_title,
                            "Bilinmeyen bağımlılık başlığı, atlanıyor"
                        );
                    }
                }
            }
        }

        // Duplicate edge temizleme
        edges.sort_unstable();
        edges.dedup();

        // 3. Topological sort (Kahn's BFS)
        let ordered_task_ids = Self::topological_sort(tasks, &edges)?;

        Ok(ResolvedDeps {
            edges,
            ordered_task_ids,
        })
    }

    /// Kahn algoritması ile topolojik sıralama.
    /// Döngü varsa `CycleDetected` döner.
    fn topological_sort(
        tasks: &[OcPlanTask],
        edges: &[(Uuid, Uuid)],
    ) -> Result<Vec<Uuid>, DependencyError> {
        // in_degree: task → kaç task'a bağımlı
        let mut in_degree: HashMap<Uuid, usize> = tasks.iter().map(|t| (t.id, 0)).collect();

        // adj: dep_id → dep_id'ye bağımlı olan task'lar (ters kenarlar BFS için)
        let mut adj: HashMap<Uuid, Vec<Uuid>> = tasks.iter().map(|t| (t.id, Vec::new())).collect();

        for &(task_id, dep_id) in edges {
            *in_degree.entry(task_id).or_insert(0) += 1;
            adj.entry(dep_id).or_default().push(task_id);
        }

        // Bağımsız task'larla başla
        let mut queue: VecDeque<Uuid> = in_degree
            .iter()
            .filter(|&(_, &d)| d == 0)
            .map(|(&id, _)| id)
            .collect();

        // Deterministik sıra için sırala (aynı bağımsızlık seviyesinde
        // orijinal order_index'e göre önce geleni seç)
        {
            let order_map: HashMap<Uuid, i64> =
                tasks.iter().map(|t| (t.id, t.order_index)).collect();
            let mut sorted: Vec<Uuid> = queue.drain(..).collect();
            sorted.sort_by_key(|id| order_map.get(id).copied().unwrap_or(i64::MAX));
            queue.extend(sorted);
        }

        let mut ordered = Vec::with_capacity(tasks.len());

        while let Some(id) = queue.pop_front() {
            ordered.push(id);
            if let Some(dependents) = adj.get(&id) {
                let mut next_batch: Vec<Uuid> = dependents
                    .iter()
                    .filter_map(|&dep| {
                        let d = in_degree.get_mut(&dep)?;
                        *d -= 1;
                        if *d == 0 { Some(dep) } else { None }
                    })
                    .collect();
                // Aynı seviyedeki task'ları order_index'e göre sırala
                let order_map: HashMap<Uuid, i64> =
                    tasks.iter().map(|t| (t.id, t.order_index)).collect();
                next_batch.sort_by_key(|id| order_map.get(id).copied().unwrap_or(i64::MAX));
                queue.extend(next_batch);
            }
        }

        if ordered.len() != tasks.len() {
            return Err(DependencyError::CycleDetected);
        }

        Ok(ordered)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::types::OcTaskComplexity;

    fn make_task(title: &str, depends_on: Vec<&str>, order_index: i64) -> OcPlanTask {
        OcPlanTask {
            id: Uuid::new_v4(),
            plan_id: Uuid::new_v4(),
            issue_id: None,
            title: title.to_string(),
            description: String::new(),
            prompt: None,
            estimated_complexity: OcTaskComplexity::Low,
            depends_on: depends_on.into_iter().map(String::from).collect(),
            order_index,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_no_deps_returns_original_order() {
        let t1 = make_task("A", vec![], 0);
        let t2 = make_task("B", vec![], 1);
        let t3 = make_task("C", vec![], 2);

        let result = PlanDependencyResolver::resolve(&[t1, t2, t3]).unwrap();
        assert!(result.edges.is_empty());
        assert_eq!(result.ordered_task_ids.len(), 3);
    }

    #[test]
    fn test_simple_chain() {
        let t1 = make_task("DB şeması", vec![], 0);
        let t2 = make_task("API endpoint", vec!["DB şeması"], 1);
        let t3 = make_task("Frontend UI", vec!["API endpoint"], 2);

        let ids = [t1.id, t2.id, t3.id];
        let result = PlanDependencyResolver::resolve(&[t1, t2, t3]).unwrap();

        // t1 ilk, t3 son olmalı
        assert_eq!(result.ordered_task_ids[0], ids[0]);
        assert_eq!(result.ordered_task_ids[1], ids[1]);
        assert_eq!(result.ordered_task_ids[2], ids[2]);
        assert_eq!(result.edges.len(), 2);
    }

    #[test]
    fn test_diamond_dependency() {
        // A → B, A → C, B → D, C → D (elmas şekli)
        let ta = make_task("A", vec![], 0);
        let tb = make_task("B", vec!["A"], 1);
        let tc = make_task("C", vec!["A"], 2);
        let td = make_task("D", vec!["B", "C"], 3);

        let result = PlanDependencyResolver::resolve(&[ta.clone(), tb, tc, td]).unwrap();

        // A ilk olmalı
        assert_eq!(result.ordered_task_ids[0], ta.id);
        assert_eq!(result.ordered_task_ids.len(), 4);
        assert_eq!(result.edges.len(), 4);
    }

    #[test]
    fn test_cycle_detected() {
        let t1 = make_task("A", vec!["B"], 0);
        let t2 = make_task("B", vec!["A"], 1);

        let result = PlanDependencyResolver::resolve(&[t1, t2]);
        assert!(matches!(result, Err(DependencyError::CycleDetected)));
    }

    #[test]
    fn test_unknown_dep_title_skipped() {
        let t1 = make_task("A", vec!["VarOlmayanTask"], 0);
        let result = PlanDependencyResolver::resolve(&[t1]).unwrap();
        // Bilinmeyen bağımlılık atlanır, sıfır kenar
        assert!(result.edges.is_empty());
        assert_eq!(result.ordered_task_ids.len(), 1);
    }

    #[test]
    fn test_self_dependency_skipped() {
        let t1 = make_task("A", vec!["A"], 0);
        let result = PlanDependencyResolver::resolve(&[t1]).unwrap();
        assert!(result.edges.is_empty());
    }
}
