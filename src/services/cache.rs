use dashmap::DashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct CacheService {
    // A thread-safe HashMap: <short_code, long_url>
    cache: Arc<DashMap<String, String>>,
}

impl CacheService {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(DashMap::new()),
        }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.cache.get(key).map(|v| v.value().clone())
    }

    pub fn set(&self, key: String, value: String) {
        self.cache.insert(key, value);
    }
}

impl Default for CacheService {
    fn default() -> Self {
        Self::new()
    }
}
