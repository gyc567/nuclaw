//! Context Cache - LRU + TTL caching for context files
//! Simple implementation using HashMap with manual LRU tracking

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

// ============================================================================
// Types
// ============================================================================

#[derive(Clone)]
struct CachedEntry {
    content: String,
    loaded_at: Instant,
    mtime: std::time::SystemTime,
}

impl CachedEntry {
    fn is_expired(&self, ttl: Duration) -> bool {
        self.loaded_at.elapsed() > ttl
    }
    
    fn is_stale(&self, path: &PathBuf) -> bool {
        if let Ok(current_mtime) = std::fs::metadata(path).and_then(|m| m.modified()) {
            self.mtime != current_mtime
        } else {
            true
        }
    }
}

// ============================================================================
// ContextCache
// ============================================================================

/// LRU + TTL cache for context files
#[derive(Clone)]
pub struct ContextCache {
    cache: Arc<RwLock<HashMap<String, CachedEntry>>>,
    access_order: Arc<RwLock<VecDeque<String>>>,
    ttl: Duration,
    max_size: usize,
}

impl ContextCache {
    /// Create a new ContextCache
    pub fn new(max_size: usize, ttl: Duration) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            access_order: Arc::new(RwLock::new(VecDeque::new())),
            ttl,
            max_size,
        }
    }
    
    /// Create with default settings (100 entries, 60s TTL)
    pub fn default_cache() -> Self {
        Self::new(100, Duration::from_secs(60))
    }
    
    /// Get or load content
    pub async fn get_or_load<F, E>(
        &self,
        key: &str,
        path: &PathBuf,
        loader: F,
    ) -> Result<String, E>
    where
        F: Future<Output = Result<String, E>>,
        E: std::fmt::Debug,
    {
        // Check cache first - need to get content and update order atomically
        let needs_load = {
            let cache = self.cache.read().await;
            match cache.get(key) {
                Some(entry) if !entry.is_expired(self.ttl) && !entry.is_stale(path) => {
                    // Update access order while holding read lock
                    drop(cache);
                    let mut order = self.access_order.write().await;
                    order.retain(|k| k != key);
                    order.push_back(key.to_string());
                    return Ok(self.cache.read().await.get(key).unwrap().content.clone());
                }
                _ => true
            }
        };
        
        if !needs_load {
            // Return cached value
            let cache = self.cache.read().await;
            return Ok(cache.get(key).unwrap().content.clone());
        }
        
        // Evict if at capacity
        self.evict_if_needed().await;
        
        // Cache miss or stale, load from source
        let content = loader.await?;
        
        // Update cache
        {
            let mut cache = self.cache.write().await;
            let mtime = std::fs::metadata(path)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            
            cache.insert(key.to_string(), CachedEntry {
                content: content.clone(),
                loaded_at: Instant::now(),
                mtime,
            });
        }
        
        // Update access order
        {
            let mut order = self.access_order.write().await;
            order.retain(|k| k != key);
            order.push_back(key.to_string());
        }
        
        Ok(content)
    }
    
    /// Evict oldest entries if cache is full
    async fn evict_if_needed(&self) {
        let len = {
            let cache = self.cache.read().await;
            cache.len()
        };
        
        if len >= self.max_size {
            let mut order = self.access_order.write().await;
            while order.len() >= self.max_size {
                if let Some(oldest) = order.pop_front() {
                    let mut cache = self.cache.write().await;
                    cache.remove(&oldest);
                }
            }
        }
    }
    
    /// Get content synchronously (for testing)
    pub async fn get(&self, key: &str) -> Option<String> {
        let cache = self.cache.read().await;
        cache.get(key).map(|e| e.content.clone())
    }
    
    /// Invalidate a specific key
    pub async fn invalidate(&self, key: &str) {
        let mut cache = self.cache.write().await;
        cache.remove(key);
        
        let mut order = self.access_order.write().await;
        order.retain(|k| k != key);
    }
    
    /// Invalidate all keys with a prefix
    pub async fn invalidate_prefix(&self, prefix: &str) {
        let keys: Vec<_> = {
            let cache = self.cache.read().await;
            cache.keys()
                .filter(|k| k.starts_with(prefix))
                .cloned()
                .collect()
        };
        
        let mut cache = self.cache.write().await;
        let mut order = self.access_order.write().await;
        
        for key in keys {
            cache.remove(&key);
            order.retain(|k| k != &key);
        }
    }
    
    /// Clear entire cache
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
        
        let mut order = self.access_order.write().await;
        order.clear();
    }
    
    /// Get cache size
    pub async fn len(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }
    
    /// Check if cache is empty
    pub async fn is_empty(&self) -> bool {
        let cache = self.cache.read().await;
        cache.is_empty()
    }
}

use std::future::Future;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{tempdir, TempDir};
    
    #[tokio::test]
    async fn test_cache_miss_loads_content() {
        let temp = tempdir().expect("Failed to create temp dir");
        let file = temp.path().join("test.txt");
        std::fs::write(&file, "content").unwrap();
        
        let cache = ContextCache::new(10, Duration::from_secs(60));
        
        let content = cache.get_or_load("key1", &file, async {
            Ok::<_, ()>(std::fs::read_to_string(&file).unwrap())
        }).await.unwrap();
        
        assert_eq!(content, "content");
    }
    
    #[tokio::test]
    async fn test_cache_hit_returns_cached() {
        let temp = tempdir().expect("Failed to create temp dir");
        let file = temp.path().join("test.txt");
        std::fs::write(&file, "original").unwrap();
        
        let cache = ContextCache::new(10, Duration::from_secs(60));
        
        // First load
        cache.get_or_load("key1", &file, async {
            Ok::<_, ()>(std::fs::read_to_string(&file).unwrap())
        }).await.unwrap();
        
        // Modify file
        std::fs::write(&file, "modified").unwrap();
        
        // Second load should return cached
        let content = cache.get_or_load("key1", &file, async {
            Ok::<_, ()>(std::fs::read_to_string(&file).unwrap())
        }).await.unwrap();
        
        assert_eq!(content, "original");
    }
    
    #[tokio::test]
    async fn test_cache_ttl_expiration() {
        let temp = tempdir().expect("Failed to create temp dir");
        let file = temp.path().join("test.txt");
        std::fs::write(&file, "v1").unwrap();
        
        let cache = ContextCache::new(10, Duration::from_millis(10));
        
        // First load
        cache.get_or_load("key1", &file, async {
            Ok::<_, ()>(std::fs::read_to_string(&file).unwrap())
        }).await.unwrap();
        
        // Wait for TTL
        tokio::time::sleep(Duration::from_millis(20)).await;
        
        // File updated
        std::fs::write(&file, "v2").unwrap();
        
        // Should load new content
        let content = cache.get_or_load("key1", &file, async {
            Ok::<_, ()>(std::fs::read_to_string(&file).unwrap())
        }).await.unwrap();
        
        assert_eq!(content, "v2");
    }
    
    #[tokio::test]
    async fn test_cache_lru_eviction() {
        let temp = tempdir().expect("Failed to create temp dir");
        let cache = ContextCache::new(2, Duration::from_secs(60));
        
        for i in 0..3 {
            let file = temp.path().join(format!("{}.txt", i));
            std::fs::write(&file, format!("content{}", i)).unwrap();
            
            cache.get_or_load(&format!("key{}", i), &file.clone(), async move {
                Ok::<_, ()>(std::fs::read_to_string(&file).unwrap())
            }).await.unwrap();
        }
        
        // Cache should have at most 2 entries (LRU eviction)
        let len = cache.len().await;
        assert!(len <= 2);
    }
    
    #[tokio::test]
    async fn test_cache_invalidate() {
        let temp = tempdir().expect("Failed to create temp dir");
        let file = temp.path().join("test.txt");
        std::fs::write(&file, "content").unwrap();
        
        let cache = ContextCache::new(10, Duration::from_secs(60));
        
        // Load and cache
        cache.get_or_load("key1", &file, async {
            Ok::<_, ()>(std::fs::read_to_string(&file).unwrap())
        }).await.unwrap();
        
        // Invalidate
        cache.invalidate("key1").await;
        
        // Should be None
        let result = cache.get("key1").await;
        assert!(result.is_none());
    }
    
    #[tokio::test]
    async fn test_cache_invalidate_prefix() {
        let temp = tempdir().expect("Failed to create temp dir");
        let cache = ContextCache::new(10, Duration::from_secs(60));
        
        // Create some cached entries
        for i in 0..3 {
            let file = temp.path().join(format!("{}.txt", i));
            std::fs::write(&file, format!("content{}", i)).unwrap();
            
            cache.get_or_load(&format!("group_{}", i), &file.clone(), async move {
                Ok::<_, ()>(std::fs::read_to_string(&file).unwrap())
            }).await.unwrap();
        }
        
        // Invalidate prefix
        cache.invalidate_prefix("group_").await;
        
        // Should all be None
        for i in 0..3 {
            let result = cache.get(&format!("group_{}", i)).await;
            assert!(result.is_none());
        }
    }
    
    #[tokio::test]
    async fn test_cache_clear() {
        let temp = tempdir().expect("Failed to create temp dir");
        let file = temp.path().join("test.txt");
        std::fs::write(&file, "content").unwrap();
        
        let cache = ContextCache::new(10, Duration::from_secs(60));
        
        cache.get_or_load("key1", &file, async {
            Ok::<_, ()>(std::fs::read_to_string(&file).unwrap())
        }).await.unwrap();
        
        cache.clear().await;
        
        assert!(cache.is_empty().await);
    }
}
