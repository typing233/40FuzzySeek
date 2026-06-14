use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use nucleo_matcher::pattern::{Atom, AtomKind, CaseMatching, Normalization};
use nucleo_matcher::{Config as NucleoConfig, Matcher, Utf32Str};
use parking_lot::RwLock;

use crate::input::SharedStore;

#[derive(Clone)]
pub struct MatchResult {
    pub index: usize,
    pub score: u32,
    pub positions: Vec<u32>,
}

pub struct MatchState {
    pub results: Vec<MatchResult>,
    pub total_scanned: usize,
    pub is_complete: bool,
    pub generation: u64,
}

pub type SharedMatchState = Arc<RwLock<MatchState>>;

pub struct FuzzyMatcher {
    store: SharedStore,
    match_state: SharedMatchState,
    cancel_flag: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
    _handle: Option<JoinHandle<()>>,
}

impl FuzzyMatcher {
    pub fn new(store: SharedStore) -> Self {
        let match_state = Arc::new(RwLock::new(MatchState {
            results: Vec::new(),
            total_scanned: 0,
            is_complete: true,
            generation: 0,
        }));

        Self {
            store,
            match_state,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            generation: Arc::new(AtomicU64::new(0)),
            _handle: None,
        }
    }

    pub fn match_state(&self) -> SharedMatchState {
        Arc::clone(&self.match_state)
    }

    pub fn update_query(&mut self, query: &str) {
        self.cancel_flag.store(true, Ordering::SeqCst);
        let gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;

        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel_flag = Arc::clone(&cancel);

        let store = Arc::clone(&self.store);
        let match_state = Arc::clone(&self.match_state);
        let query = query.to_string();
        let generation = Arc::clone(&self.generation);

        self._handle = Some(thread::spawn(move || {
            if query.is_empty() {
                // No query: show all items in order (no score needed)
                let s = store.read();
                let total = s.len();
                let is_done = s.is_done();
                drop(s);

                let results: Vec<MatchResult> = (0..total)
                    .map(|i| MatchResult {
                        index: i,
                        score: 0,
                        positions: Vec::new(),
                    })
                    .collect();

                let mut ms = match_state.write();
                if generation.load(Ordering::SeqCst) == gen {
                    ms.results = results;
                    ms.total_scanned = total;
                    ms.is_complete = is_done;
                    ms.generation = gen;
                }
                return;
            }

            let atom = Atom::new(
                &query,
                CaseMatching::Smart,
                Normalization::Smart,
                AtomKind::Fuzzy,
                false,
            );

            let mut matcher = Matcher::new(NucleoConfig::DEFAULT);
            let mut results: Vec<MatchResult> = Vec::new();
            let mut buf = Vec::new();

            let scan_chunk = 8192;
            let mut offset = 0;

            loop {
                if cancel.load(Ordering::Relaxed) {
                    return;
                }

                let total_available: usize;
                let is_done: bool;
                let items: Vec<Arc<str>>;
                {
                    let s = store.read();
                    total_available = s.len();
                    is_done = s.is_done();
                    let end = total_available.min(offset + scan_chunk);
                    if offset >= end {
                        if is_done {
                            break;
                        }
                        drop(s);
                        thread::sleep(std::time::Duration::from_millis(30));
                        continue;
                    }
                    items = s.get_range(offset, end);
                }

                let batch_len = items.len();
                for (i, item) in items.iter().enumerate() {
                    if cancel.load(Ordering::Relaxed) {
                        return;
                    }

                    let stripped = strip_ansi(item);
                    let haystack = Utf32Str::new(&stripped, &mut buf);
                    let mut indices = Vec::new();
                    if let Some(score) = atom.indices(haystack, &mut matcher, &mut indices) {
                        indices.sort_unstable();
                        indices.dedup();
                        results.push(MatchResult {
                            index: offset + i,
                            score: score as u32,
                            positions: indices,
                        });
                    }
                    buf.clear();
                }

                offset += batch_len;

                // Stable sort: score desc, then index asc
                results.sort_by(|a, b| {
                    b.score.cmp(&a.score).then_with(|| a.index.cmp(&b.index))
                });

                // Swap results into shared state (only the index+score+positions, not string data)
                {
                    let mut ms = match_state.write();
                    if generation.load(Ordering::SeqCst) != gen {
                        return;
                    }
                    // Swap to avoid full clone: put new results in, take old allocation back
                    let mut new_results = results;
                    std::mem::swap(&mut ms.results, &mut new_results);
                    results = new_results;
                    // Now `results` has the old allocation, `ms.results` has current data
                    // Rebuild results from ms since we swapped
                    results.clear();
                    results.extend_from_slice(&ms.results);
                    ms.total_scanned = offset;
                    ms.is_complete = is_done && offset >= total_available;
                    ms.generation = gen;
                }

                if is_done && offset >= total_available {
                    break;
                }
            }

            // Final update
            let s = store.read();
            let mut ms = match_state.write();
            if generation.load(Ordering::SeqCst) == gen {
                ms.is_complete = s.is_done();
            }
        }));
    }
}

pub fn strip_ansi(s: &str) -> String {
    let bytes = strip_ansi_escapes::strip(s);
    String::from_utf8_lossy(&bytes).into_owned()
}
