use super::PreviewContent;
use std::collections::HashMap;

pub struct PreviewCache {
    entries: HashMap<String, (PreviewContent, u64)>,
    access_counter: u64,
    capacity: usize,
}

impl PreviewCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(capacity.min(256)),
            access_counter: 0,
            capacity,
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&PreviewContent> {
        if let Some(entry) = self.entries.get_mut(key) {
            self.access_counter += 1;
            entry.1 = self.access_counter;
            Some(&entry.0)
        } else {
            None
        }
    }

    pub fn insert(&mut self, key: String, content: PreviewContent) {
        self.access_counter += 1;
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.evict_lru();
        }
        self.entries.insert(key, (content, self.access_counter));
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.access_counter = 0;
    }

    fn evict_lru(&mut self) {
        if let Some(lru_key) = self.entries.iter()
            .min_by_key(|(_, (_, ts))| ts)
            .map(|(k, _)| k.clone())
        {
            self.entries.remove(&lru_key);
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_get() {
        let mut cache = PreviewCache::new(10);
        cache.insert("key1".to_string(), PreviewContent::Text("hello".to_string()));
        assert!(cache.get("key1").is_some());
        assert!(cache.get("key2").is_none());
    }

    #[test]
    fn test_lru_eviction() {
        let mut cache = PreviewCache::new(3);
        cache.insert("a".to_string(), PreviewContent::Text("1".to_string()));
        cache.insert("b".to_string(), PreviewContent::Text("2".to_string()));
        cache.insert("c".to_string(), PreviewContent::Text("3".to_string()));

        // Access "a" to make it recently used
        cache.get("a");

        // Insert "d" should evict "b" (least recently used)
        cache.insert("d".to_string(), PreviewContent::Text("4".to_string()));

        assert!(cache.get("a").is_some());
        assert!(cache.get("b").is_none()); // evicted
        assert!(cache.get("c").is_some());
        assert!(cache.get("d").is_some());
    }

    #[test]
    fn test_overwrite_existing() {
        let mut cache = PreviewCache::new(3);
        cache.insert("key".to_string(), PreviewContent::Text("v1".to_string()));
        cache.insert("key".to_string(), PreviewContent::Text("v2".to_string()));
        assert_eq!(cache.len(), 1);
        if let Some(PreviewContent::Text(s)) = cache.get("key") {
            assert_eq!(s, "v2");
        } else {
            panic!("expected text content");
        }
    }

    #[test]
    fn test_clear() {
        let mut cache = PreviewCache::new(10);
        cache.insert("a".to_string(), PreviewContent::Text("1".to_string()));
        cache.insert("b".to_string(), PreviewContent::Text("2".to_string()));
        cache.clear();
        assert_eq!(cache.len(), 0);
        assert!(cache.get("a").is_none());
    }
}
