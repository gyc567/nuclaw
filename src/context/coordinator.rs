//! Agent Coordinator - Manages multi-agent execution order

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};

// ============================================================================
// AgentCoordinator
// ============================================================================

/// Coordinates multiple agents with dependency ordering
pub struct AgentCoordinator {
    /// Dependency graph: agent -> agents it depends on
    dependencies: Arc<RwLock<HashMap<String, Vec<String>>>>,
    /// File locks for single-writer pattern
    file_locks: Arc<RwLock<HashMap<String, Arc<Mutex<()>>>>>,
}

impl AgentCoordinator {
    /// Create a new AgentCoordinator
    pub fn new() -> Self {
        Self {
            dependencies: Arc::new(RwLock::new(HashMap::new())),
            file_locks: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Register a dependency (agent depends on depends_on)
    pub async fn register_dependency(&self, agent: &str, depends_on: &[String]) {
        let mut deps = self.dependencies.write().await;
        deps.insert(agent.to_string(), depends_on.to_vec());
    }
    
    /// Get execution order using topological sort
    pub async fn get_execution_order(&self, agents: &[String]) -> Vec<String> {
        let deps = self.dependencies.read().await;
        
        // Build in-degree map
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();
        
        for agent in agents {
            in_degree.insert(agent.clone(), 0);
        }
        
        // Build graph based on dependencies
        for agent in agents {
            if let Some(deps) = deps.get(agent) {
                for dep in deps {
                    if agents.contains(&dep) {
                        // dep must run before agent
                        graph.entry(dep.clone()).or_default().push(agent.clone());
                        *in_degree.entry(agent.clone()).or_insert(0) += 1;
                    }
                }
            }
        }
        
        // Topological sort (Kahn's algorithm)
        let mut queue: VecDeque<_> = in_degree.iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(k, _)| k.clone())
            .collect();
        
        let mut result = Vec::new();
        
        while let Some(node) = queue.pop_front() {
            result.push(node.clone());
            
            if let Some(neighbors) = graph.get(&node) {
                for neighbor in neighbors {
                    if let Some(deg) = in_degree.get_mut(neighbor) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(neighbor.clone());
                        }
                    }
                }
            }
        }
        
        // If result doesn't contain all agents, there's a cycle
        // Return what we have plus remaining agents
        for agent in agents {
            if !result.contains(agent) {
                result.push(agent.clone());
            }
        }
        
        result
    }
    
    /// Acquire lock for a shared resource (single-writer pattern)
    pub async fn acquire_lock(&self, resource: &str) -> Arc<Mutex<()>> {
        let mut locks = self.file_locks.write().await;
        
        if let Some(lock) = locks.get(resource) {
            return Arc::clone(lock);
        }
        
        let lock = Arc::new(Mutex::new(()));
        locks.insert(resource.to_string(), Arc::clone(&lock));
        lock
    }
}

// Implement Default
impl Default for AgentCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_no_dependencies() {
        let coordinator = AgentCoordinator::new();
        
        let order = coordinator.get_execution_order(&["a".to_string(), "b".to_string()]).await;
        
        assert_eq!(order.len(), 2);
    }
    
    #[tokio::test]
    async fn test_linear_dependency() {
        let coordinator = AgentCoordinator::new();
        
        coordinator.register_dependency("c", &["b".to_string()]).await;
        coordinator.register_dependency("b", &["a".to_string()]).await;
        
        let order = coordinator.get_execution_order(&["c".to_string(), "b".to_string(), "a".to_string()]).await;
        
        // a should come first, then b, then c
        let a_pos = order.iter().position(|x| x == "a").unwrap();
        let b_pos = order.iter().position(|x| x == "b").unwrap();
        let c_pos = order.iter().position(|x| x == "c").unwrap();
        
        assert!(a_pos < b_pos);
        assert!(b_pos < c_pos);
    }
    
    #[tokio::test]
    async fn test_parallel_branches() {
        let coordinator = AgentCoordinator::new();
        
        // a -> b, c -> b (both branches can run in parallel)
        coordinator.register_dependency("b", &["a".to_string()]).await;
        coordinator.register_dependency("b", &["c".to_string()]).await;
        
        let order = coordinator.get_execution_order(&["b".to_string(), "a".to_string(), "c".to_string()]).await;
        
        // b should be last
        let b_pos = order.iter().position(|x| x == "b").unwrap();
        assert!(b_pos == 2);
    }
    
    #[tokio::test]
    async fn test_register_dependency() {
        let coordinator = AgentCoordinator::new();
        
        coordinator.register_dependency("agent1", &["dep1".to_string(), "dep2".to_string()]).await;
        
        let deps = coordinator.dependencies.read().await;
        let agent_deps = deps.get("agent1").unwrap();
        
        assert_eq!(agent_deps.len(), 2);
        assert!(agent_deps.contains(&"dep1".to_string()));
        assert!(agent_deps.contains(&"dep2".to_string()));
    }
    
    #[tokio::test]
    async fn test_acquire_lock() {
        let coordinator = AgentCoordinator::new();
        
        let lock1 = coordinator.acquire_lock("resource1").await;
        let lock2 = coordinator.acquire_lock("resource1").await;
        
        // Same resource should return same lock
        let ptr1 = Arc::as_ptr(&lock1);
        let ptr2 = Arc::as_ptr(&lock2);
        assert_eq!(ptr1, ptr2);
        
        // Different resource
        let lock3 = coordinator.acquire_lock("resource2").await;
        let ptr3 = Arc::as_ptr(&lock3);
        assert_ne!(ptr1, ptr3);
    }
    
    #[tokio::test]
    async fn test_circular_dependency() {
        let coordinator = AgentCoordinator::new();
        
        // a -> b, b -> a (cycle)
        coordinator.register_dependency("a", &["b".to_string()]).await;
        coordinator.register_dependency("b", &["a".to_string()]).await;
        
        let order = coordinator.get_execution_order(&["a".to_string(), "b".to_string()]).await;
        
        // Should still return both (break cycle)
        assert_eq!(order.len(), 2);
    }
}
