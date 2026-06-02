use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct CacheEntry {
    value: String,
    created_at: Instant,
    expires_at: Option<i64>,
}

#[derive(Clone)]
pub struct CacheService {
    cache: Arc<DashMap<String, CacheEntry>>,
    ttl: Duration,
}

impl CacheService {
    pub fn new(ttl: Duration) -> Self {
        Self {
            cache: Arc::new(DashMap::new()),
            ttl,
        }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let entry = self.cache.get(key)?;
        if entry.created_at.elapsed() >= self.ttl {
            return None;
        }
        if let Some(expires_at) = entry.expires_at {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            if now >= expires_at {
                return None;
            }
        }
        Some(entry.value.clone())
    }

    pub fn set(&self, key: String, value: String) {
        self.set_with_expiry(key, value, None);
    }

    pub fn set_with_expiry(&self, key: String, value: String, expires_at: Option<i64>) {
        self.cache.insert(
            key,
            CacheEntry {
                value,
                created_at: Instant::now(),
                expires_at,
            },
        );
    }

    pub fn invalidate(&self, key: &str) {
        self.cache.remove(key);
    }

    pub fn evict_expired(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        self.cache.retain(|_, entry| {
            if entry.created_at.elapsed() >= self.ttl {
                return false;
            }
            if let Some(expires_at) = entry.expires_at {
                if now >= expires_at {
                    return false;
                }
            }
            true
        });
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }
}

impl Default for CacheService {
    fn default() -> Self {
        Self::new(Duration::from_secs(3600))
    }
}

