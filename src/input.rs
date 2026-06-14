use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::sync::Arc;
use std::thread;

use parking_lot::RwLock;

#[derive(Clone)]
pub enum InputSource {
    Stdin,
    File(String),
}

const CHUNK_SIZE: usize = 16384;

/// Chunked storage: items stored in fixed-size chunks to avoid reallocation
/// of a single huge Vec. Each chunk is Arc'd for zero-copy sharing.
struct Chunk {
    lines: Vec<Arc<str>>,
}

pub struct ItemStore {
    chunks: Vec<Chunk>,
    total: usize,
    done: bool,
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

    /// Get a slice of items by index range without copying strings.
    /// Returns Arc<str> references.
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
}

pub type SharedStore = Arc<RwLock<ItemStore>>;

pub fn start_reader(source: InputSource) -> Result<SharedStore, io::Error> {
    let store = Arc::new(RwLock::new(ItemStore::new()));
    let store_clone = Arc::clone(&store);

    let reader: Box<dyn BufRead + Send> = match source {
        InputSource::Stdin => Box::new(BufReader::with_capacity(256 * 1024, io::stdin())),
        InputSource::File(ref path) => {
            let file = File::open(path)?;
            Box::new(BufReader::with_capacity(256 * 1024, file))
        }
    };

    thread::spawn(move || {
        let mut batch: Vec<Arc<str>> = Vec::with_capacity(CHUNK_SIZE);

        for line in reader.lines() {
            match line {
                Ok(l) => {
                    batch.push(Arc::from(l.into_boxed_str()));
                    if batch.len() >= CHUNK_SIZE {
                        let chunk = Chunk { lines: std::mem::take(&mut batch) };
                        let mut s = store_clone.write();
                        s.total += chunk.lines.len();
                        s.chunks.push(chunk);
                        drop(s);
                        batch = Vec::with_capacity(CHUNK_SIZE);
                    }
                }
                Err(_) => break,
            }
        }

        let mut s = store_clone.write();
        if !batch.is_empty() {
            let count = batch.len();
            s.chunks.push(Chunk { lines: batch });
            s.total += count;
        }
        s.done = true;
    });

    Ok(store)
}
