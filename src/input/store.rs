use std::sync::Arc;
use parking_lot::RwLock;

pub const CHUNK_SIZE: usize = 16384;

pub struct Chunk {
    pub lines: Vec<Arc<str>>,
}

pub struct ItemStore {
    pub chunks: Vec<Chunk>,
    pub total: usize,
    pub done: bool,
}

impl ItemStore {
    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
            total: 0,
            done: false,
        }
    }

    pub fn len(&self) -> usize {
        self.total
    }

    pub fn is_done(&self) -> bool {
        self.done
    }

    pub fn get(&self, index: usize) -> Option<&Arc<str>> {
        let chunk_idx = index / CHUNK_SIZE;
        let inner_idx = index % CHUNK_SIZE;
        self.chunks.get(chunk_idx)?.lines.get(inner_idx)
    }

    pub fn get_range(&self, start: usize, end: usize) -> Vec<Arc<str>> {
        let end = end.min(self.total);
        let mut result = Vec::with_capacity(end - start);
        for i in start..end {
            if let Some(s) = self.get(i) {
                result.push(Arc::clone(s));
            }
        }
        result
    }

    pub fn push_batch(&mut self, batch: Vec<Arc<str>>) {
        let count = batch.len();
        self.chunks.push(Chunk { lines: batch });
        self.total += count;
    }
}

pub type SharedStore = Arc<RwLock<ItemStore>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_store() {
        let store = ItemStore::new();
        assert_eq!(store.len(), 0);
        assert!(store.is_done() == false);
        assert!(store.get(0).is_none());
    }

    #[test]
    fn test_push_and_get() {
        let mut store = ItemStore::new();
        let batch: Vec<Arc<str>> = (0..10).map(|i| Arc::from(format!("line{}", i))).collect();
        store.push_batch(batch);
        assert_eq!(store.len(), 10);
        assert_eq!(store.get(0).unwrap().as_ref(), "line0");
        assert_eq!(store.get(9).unwrap().as_ref(), "line9");
        assert!(store.get(10).is_none());
    }

    #[test]
    fn test_chunk_boundary() {
        let mut store = ItemStore::new();
        let batch1: Vec<Arc<str>> = (0..CHUNK_SIZE).map(|i| Arc::from(format!("{}", i))).collect();
        let batch2: Vec<Arc<str>> = vec![Arc::from("overflow")];
        store.push_batch(batch1);
        store.push_batch(batch2);
        assert_eq!(store.len(), CHUNK_SIZE + 1);
        assert_eq!(store.get(CHUNK_SIZE).unwrap().as_ref(), "overflow");
    }

    #[test]
    fn test_get_range() {
        let mut store = ItemStore::new();
        let batch: Vec<Arc<str>> = (0..100).map(|i| Arc::from(format!("item{}", i))).collect();
        store.push_batch(batch);
        let range = store.get_range(5, 10);
        assert_eq!(range.len(), 5);
        assert_eq!(range[0].as_ref(), "item5");
        assert_eq!(range[4].as_ref(), "item9");
    }

    #[test]
    fn test_get_range_clamped() {
        let mut store = ItemStore::new();
        let batch: Vec<Arc<str>> = (0..5).map(|i| Arc::from(format!("{}", i))).collect();
        store.push_batch(batch);
        let range = store.get_range(3, 100);
        assert_eq!(range.len(), 2);
    }
}
