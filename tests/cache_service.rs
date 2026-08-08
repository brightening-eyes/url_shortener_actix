use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use url_shortener::services::cache::CacheService;

#[test]
fn test_set_and_get() {
    let cache = CacheService::new(Duration::from_secs(60));
    cache.set("abc".to_string(), "https://example.com".to_string());
    assert_eq!(cache.get("abc"), Some("https://example.com".to_string()));
}

#[test]
fn test_get_missing_key() {
    let cache = CacheService::new(Duration::from_secs(60));
    assert_eq!(cache.get("nonexistent"), None);
}

#[test]
fn test_ttl_expiry() {
    let cache = CacheService::new(Duration::from_millis(10));
    cache.set("abc".to_string(), "https://example.com".to_string());
    assert_eq!(cache.get("abc"), Some("https://example.com".to_string()));
    thread::sleep(Duration::from_millis(20));
    assert_eq!(cache.get("abc"), None);
}

#[test]
fn test_custom_expiry() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let cache = CacheService::new(Duration::from_secs(60));

    cache.set_with_expiry("expires_soon".to_string(), "https://example.com".to_string(), Some(now + 1));
    assert_eq!(cache.get("expires_soon"), Some("https://example.com".to_string()));

    thread::sleep(Duration::from_secs(2));
    assert_eq!(cache.get("expires_soon"), None);
}

#[test]
fn test_set_overwrites() {
    let cache = CacheService::new(Duration::from_secs(60));
    cache.set("key".to_string(), "value1".to_string());
    cache.set("key".to_string(), "value2".to_string());
    assert_eq!(cache.get("key"), Some("value2".to_string()));
}

#[test]
fn test_invalidate() {
    let cache = CacheService::new(Duration::from_secs(60));
    cache.set("key".to_string(), "value".to_string());
    assert_eq!(cache.get("key"), Some("value".to_string()));
    cache.invalidate("key");
    assert_eq!(cache.get("key"), None);
}

#[test]
fn test_len() {
    let cache = CacheService::new(Duration::from_secs(60));
    assert_eq!(cache.len(), 0);
    cache.set("a".to_string(), "1".to_string());
    cache.set("b".to_string(), "2".to_string());
    assert_eq!(cache.len(), 2);
}

#[test]
fn test_evict_expired_ttl() {
    let cache = CacheService::new(Duration::from_millis(10));
    cache.set("a".to_string(), "1".to_string());
    cache.set("b".to_string(), "2".to_string());
    thread::sleep(Duration::from_millis(20));
    cache.evict_expired();
    assert_eq!(cache.len(), 0);
}

#[test]
fn test_evict_expired_custom_expiry() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let cache = CacheService::new(Duration::from_secs(60));

    cache.set("permanent".to_string(), "value".to_string());
    cache.set_with_expiry("temporary".to_string(), "value".to_string(), Some(now - 1));

    cache.evict_expired();
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.get("permanent"), Some("value".to_string()));
    assert_eq!(cache.get("temporary"), None);
}

#[test]
fn test_ttl_takes_precedence_over_custom_expiry() {
    let cache = CacheService::new(Duration::from_millis(10));
    let far_future = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
        + 3600;
    cache.set_with_expiry("key".to_string(), "value".to_string(), Some(far_future));
    thread::sleep(Duration::from_millis(20));
    assert_eq!(cache.get("key"), None);
}
