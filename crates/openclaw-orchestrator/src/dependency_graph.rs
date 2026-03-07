use std::collections::{HashMap, HashSet};

use uuid::Uuid;

/// Görevler arası bağımlılık grafiği (Directed Acyclic Graph).
/// task_id → bu task'ın bağlı olduğu task ID'leri kümesi
#[derive(Debug, Default)]
pub struct DependencyGraph {
    /// task_id → bağımlı olduğu task_id'ler (önceden tamamlanması gerekenler)
    deps: HashMap<Uuid, HashSet<Uuid>>,
    /// Tüm bilinen task'lar
    all_tasks: HashSet<Uuid>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Yeni bir task ekler (bağımlılık olmadan).
    pub fn add_task(&mut self, task_id: Uuid) {
        self.all_tasks.insert(task_id);
        self.deps.entry(task_id).or_default();
    }

    /// task_id'nin depends_on_id'den önce başlayamayacağını belirtir.
    pub fn add_dependency(&mut self, task_id: Uuid, depends_on_id: Uuid) {
        self.all_tasks.insert(task_id);
        self.all_tasks.insert(depends_on_id);
        self.deps.entry(task_id).or_default().insert(depends_on_id);
        self.deps.entry(depends_on_id).or_default();
    }

    /// Tamamlanan task'ları set olarak al, hangi task'lar artık hazır?
    pub fn ready_tasks(&self, completed: &HashSet<Uuid>) -> Vec<Uuid> {
        self.all_tasks
            .iter()
            .filter(|task_id| {
                if completed.contains(*task_id) {
                    return false; // Zaten tamamlandı
                }
                // Tüm bağımlılıkları tamamlandı mı?
                self.deps
                    .get(*task_id)
                    .map(|deps| deps.iter().all(|dep| completed.contains(dep)))
                    .unwrap_or(true)
            })
            .copied()
            .collect()
    }

    /// Belirli bir task'ı bloklayan tamamlanmamış bağımlılıkları döner.
    pub fn blocking_tasks(&self, task_id: Uuid, completed: &HashSet<Uuid>) -> Vec<Uuid> {
        self.deps
            .get(&task_id)
            .map(|deps| {
                deps.iter()
                    .filter(|dep| !completed.contains(*dep))
                    .copied()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Döngüsel bağımlılık var mı kontrol eder (DFS tabanlı).
    pub fn has_cycle(&self) -> bool {
        let mut visited = HashSet::new();
        let mut stack = HashSet::new();

        for &task_id in &self.all_tasks {
            if self.dfs_cycle(task_id, &mut visited, &mut stack) {
                return true;
            }
        }
        false
    }

    fn dfs_cycle(
        &self,
        node: Uuid,
        visited: &mut HashSet<Uuid>,
        stack: &mut HashSet<Uuid>,
    ) -> bool {
        if stack.contains(&node) {
            return true;
        }
        if visited.contains(&node) {
            return false;
        }
        visited.insert(node);
        stack.insert(node);

        if let Some(deps) = self.deps.get(&node) {
            for &dep in deps {
                if self.dfs_cycle(dep, visited, stack) {
                    return true;
                }
            }
        }
        stack.remove(&node);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ready_tasks_no_deps() {
        let mut graph = DependencyGraph::new();
        let t1 = Uuid::new_v4();
        let t2 = Uuid::new_v4();
        graph.add_task(t1);
        graph.add_task(t2);

        let ready = graph.ready_tasks(&HashSet::new());
        assert_eq!(ready.len(), 2);
    }

    #[test]
    fn test_ready_tasks_with_dep() {
        let mut graph = DependencyGraph::new();
        let t1 = Uuid::new_v4();
        let t2 = Uuid::new_v4();
        graph.add_dependency(t2, t1); // t2 depends on t1

        let ready = graph.ready_tasks(&HashSet::new());
        assert_eq!(ready.len(), 1);
        assert!(ready.contains(&t1));

        let mut completed = HashSet::new();
        completed.insert(t1);
        let ready2 = graph.ready_tasks(&completed);
        assert!(ready2.contains(&t2));
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph = DependencyGraph::new();
        let t1 = Uuid::new_v4();
        let t2 = Uuid::new_v4();
        graph.add_dependency(t1, t2);
        graph.add_dependency(t2, t1); // döngü
        assert!(graph.has_cycle());
    }

    #[test]
    fn test_no_cycle() {
        let mut graph = DependencyGraph::new();
        let t1 = Uuid::new_v4();
        let t2 = Uuid::new_v4();
        let t3 = Uuid::new_v4();
        graph.add_dependency(t2, t1);
        graph.add_dependency(t3, t2);
        assert!(!graph.has_cycle());
    }

    #[test]
    fn test_blocking_tasks() {
        let mut graph = DependencyGraph::new();
        let t1 = Uuid::new_v4();
        let t2 = Uuid::new_v4();
        graph.add_dependency(t2, t1);

        let blocking = graph.blocking_tasks(t2, &HashSet::new());
        assert_eq!(blocking, vec![t1]);

        let mut completed = HashSet::new();
        completed.insert(t1);
        let blocking2 = graph.blocking_tasks(t2, &completed);
        assert!(blocking2.is_empty());
    }
}
