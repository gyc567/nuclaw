//! Access Tracker - Track access frequency for preloading

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// ============================================================================
// AccessTracker
// ============================================================================

/// Tracks group access frequency for intelligent preloading
#[derive(Clone)]
pub struct AccessTracker {
    frequencies: Arc<RwLock<HashMap<String, u32>>>,
    max_entries: usize,
}

impl AccessTracker {
    /// Create a new AccessTracker
    pub fn new(max_entries: usize) -> Self {
        Self {
            frequencies: Arc::new(RwLock::new(HashMap::new())),
            max_entries,
        }
    }
    
    /// Create with default settings
    pub fn default_tracker() -> Self {
        Self::new(100)
    }
    
    /// Record an access to a group
    pub async fn record(&self, group: &str) {
        // First, get the current frequency
        let new_count = {
            let frequencies = self.frequencies.read().await;
            *frequencies.get(group).unwrap_or(&0) + 1
        };
        
        // Then update
        {
            let mut frequencies = self.frequencies.write().await;
            frequencies.insert(group.to_string(), new_count);
        }
        
        // Trim if too large
        self.trim_if_needed().await;
    }
    
    /// Trim entries if too large
    async fn trim_if_needed(&self) {
        let len = {
            let frequencies = self.frequencies.read().await;
            frequencies.len()
        };
        
        if len > self.max_entries {
            let mut frequencies = self.frequencies.write().await;
            
            // Keep top entries
            let mut items: Vec<_> = frequencies.iter().map(|(k, v)| (k.clone(), *v)).collect();
            items.sort_by(|a, b| b.1.cmp(&a.1));
            items.truncate(self.max_entries);
            
            frequencies.clear();
            for (k, v) in items {
                frequencies.insert(k, v);
            }
        }
    }
    
    /// Get the most accessed groups
    pub async fn get_top(&self, n: usize) -> Vec<String> {
        let frequencies = self.frequencies.read().await;
        
        let mut items: Vec<_> = frequencies.iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        
        items.sort_by(|a, b| b.1.cmp(&a.1));
        items.into_iter()
            .take(n)
            .map(|(k, _)| k)
            .collect()
    }
    
    /// Get frequency for a specific group
    pub async fn get_frequency(&self, group: &str) -> u32 {
        let frequencies = self.frequencies.read().await;
        *frequencies.get(group).unwrap_or(&0)
    }
    
    /// Get all groups sorted by frequency
    pub async fn get_all_sorted(&self) -> Vec<(String, u32)> {
        let frequencies = self.frequencies.read().await;
        
        let mut items: Vec<_> = frequencies.iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        
        items.sort_by(|a, b| b.1.cmp(&a.1));
        items
    }
    
    /// Clear all tracking data
    pub async fn clear(&self) {
        let mut frequencies = self.frequencies.write().await;
        frequencies.clear();
    }
    
    /// Get total tracked groups
    pub async fn len(&self) -> usize {
        let frequencies = self.frequencies.read().await;
        frequencies.len()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_record_single() {
        let tracker = AccessTracker::new(10);
        
        tracker.record("group1").await;
        
        let freq = tracker.get_frequency("group1").await;
        assert_eq!(freq, 1);
    }
    
    #[tokio::test]
    async fn test_record_multiple() {
        let tracker = AccessTracker::new(10);
        
        tracker.record("group1").await;
        tracker.record("group1").await;
        tracker.record("group2").await;
        
        assert_eq!(tracker.get_frequency("group1").await, 2);
        assert_eq!(tracker.get_frequency("group2").await, 1);
    }
    
    #[tokio::test]
    async fn test_get_top() {
        let tracker = AccessTracker::new(10);
        
        tracker.record("group1").await;
        tracker.record("group1").await;
        tracker.record("group1").await;
        
        tracker.record("group2").await;
        tracker.record("group2").await;
        
        tracker.record("group3").await;
        
        let top = tracker.get_top(2).await;
        
        assert_eq!(top[0], "group1");
        assert_eq!(top[1], "group2");
    }
    
    #[tokio::test]
    async fn test_get_all_sorted() {
        let tracker = AccessTracker::new(10);
        
        tracker.record("a").await;
        tracker.record("b").await;
        tracker.record("a").await;
        tracker.record("c").await;
        
        let sorted = tracker.get_all_sorted().await;
        
        assert_eq!(sorted[0].0, "a");
        assert_eq!(sorted[0].1, 2);
    }
    
    #[tokio::test]
    async fn test_clear() {
        let tracker = AccessTracker::new(10);
        
        tracker.record("group1").await;
        tracker.clear().await;
        
        assert_eq!(tracker.len().await, 0);
    }
    
    #[tokio::test]
    async fn test_nonexistent_group() {
        let tracker = AccessTracker::new(10);
        
        let freq = tracker.get_frequency("nonexistent").await;
        assert_eq!(freq, 0);
    }
}
