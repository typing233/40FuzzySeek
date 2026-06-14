use std::sync::Arc;
use parking_lot::RwLock;

pub const CHUNK_SIZE: usize = 16384;

pub struct Chunk {
    pub lines: Vec<Arc<str>>,
}

/// Stores parsed metadata for items when a structured parser is active.
/// For each item at index i, search_texts[i] is what the matcher searches against,
/// and output_texts[i] is what gets emitted on selection.
pub struct ParsedMeta {
    pub search_texts: Vec<Arc<str>>,
    pub output_texts: Vec<Arc<str>>,
}

impl ParsedMeta {
    pub fn new() -> Self {
        Self {
            search_texts: Vec::new(),
            output_texts: Vec::new(),
        }
    }
}

pub struct ItemStore {
    pub chunks: Vec<Chunk>,
    pub total: usize,
    pub done: bool,
    pub parsed: Option<ParsedMeta>,
}

impl ItemStore {
    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
            total: 0,
            done: false,
            parsed: None,
        }
    }

    pub fn new_with_parser() -> Self {
        Self {
            chunks: Vec::new(),
            total: 0,
            done: false,
            parsed: Some(ParsedMeta::new()),
        }
    }

    pub fn len(&self) -> usize {
        self.total
    }

    pub fn is_done(&self) -> bool {
        self.done
    }

    /// Get the display text (raw line) at index
    pub fn get(&self, index: usize) -> Option<&Arc<str>> {
        let chunk_idx = index / CHUNK_SIZE;
        let inner_idx = index % CHUNK_SIZE;
        self.chunks.get(chunk_idx)?.lines.get(inner_idx)
    }

    /// Get the text to match against (search_text if parsed, else raw line)
    pub fn get_search_text(&self, index: usize) -> Option<&Arc<str>> {
        if let Some(ref parsed) = self.parsed {
            parsed.search_texts.get(index)
        } else {
            self.get(index)
        }
    }

    /// Get the text to output on selection (output_text if parsed, else raw line)
    pub fn get_output_text(&self, index: usize) -> Option<&Arc<str>> {
        if let Some(ref parsed) = self.parsed {
            parsed.output_texts.get(index)
        } else {
            self.get(index)
        }
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

    /// Get search texts for a range (uses search_text if parser active)
    pub fn get_search_range(&self, start: usize, end: usize) -> Vec<Arc<str>> {
        let end = end.min(self.total);
        let mut result = Vec::with_capacity(end - start);
        for i in start..end {
            if let Some(s) = self.get_search_text(i) {
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

    pub fn push_batch_parsed(
        &mut self,
        display_lines: Vec<Arc<str>>,
        search_texts: Vec<Arc<str>>,
        output_texts: Vec<Arc<str>>,
    ) {
        let count = display_lines.len();
        self.chunks.push(Chunk { lines: display_lines });
        if let Some(ref mut parsed) = self.parsed {
            parsed.search_texts.extend(search_texts);
            parsed.output_texts.extend(output_texts);
        }
        self.total += count;
    }

    pub fn has_parser(&self) -> bool {
        self.parsed.is_some()
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

    #[test]
    fn test_parsed_store() {
        let mut store = ItemStore::new_with_parser();
        let display = vec![
            Arc::from("  1  git status"),
            Arc::from("  2  cd /tmp"),
        ];
        let search = vec![
            Arc::from("git status"),
            Arc::from("cd /tmp"),
        ];
        let output = vec![
            Arc::from("git status"),
            Arc::from("cd /tmp"),
        ];
        store.push_batch_parsed(display, search, output);

        assert_eq!(store.len(), 2);
        // Display shows full line
        assert_eq!(store.get(0).unwrap().as_ref(), "  1  git status");
        // Search matches against parsed command
        assert_eq!(store.get_search_text(0).unwrap().as_ref(), "git status");
        // Output returns the command
        assert_eq!(store.get_output_text(0).unwrap().as_ref(), "git status");
    }

    #[test]
    fn test_unparsed_store_search_equals_display() {
        let mut store = ItemStore::new();
        let batch = vec![Arc::from("hello world")];
        store.push_batch(batch);
        // Without parser, all three return the same thing
        assert_eq!(store.get(0).unwrap().as_ref(), "hello world");
        assert_eq!(store.get_search_text(0).unwrap().as_ref(), "hello world");
        assert_eq!(store.get_output_text(0).unwrap().as_ref(), "hello world");
    }
}
