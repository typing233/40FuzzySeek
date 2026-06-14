use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::sync::Arc;
use std::thread;

use parking_lot::Mutex;

#[derive(Clone)]
pub enum InputSource {
    Stdin,
    File(String),
}

pub struct ItemStore {
    items: Vec<String>,
    done: bool,
}

impl ItemStore {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            done: false,
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_done(&self) -> bool {
        self.done
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.items.get(index).map(|s| s.as_str())
    }

    pub fn items(&self) -> &[String] {
        &self.items
    }
}

pub type SharedStore = Arc<Mutex<ItemStore>>;

pub fn start_reader(source: InputSource) -> SharedStore {
    let store = Arc::new(Mutex::new(ItemStore::new()));
    let store_clone = Arc::clone(&store);

    thread::spawn(move || {
        let reader: Box<dyn BufRead + Send> = match source {
            InputSource::Stdin => Box::new(BufReader::with_capacity(256 * 1024, io::stdin())),
            InputSource::File(path) => {
                let file = File::open(&path).expect("failed to open input file");
                Box::new(BufReader::with_capacity(256 * 1024, file))
            }
        };

        let mut batch = Vec::with_capacity(4096);
        for line in reader.lines() {
            match line {
                Ok(l) => {
                    batch.push(l);
                    if batch.len() >= 4096 {
                        let mut s = store_clone.lock();
                        s.items.append(&mut batch);
                        batch = Vec::with_capacity(4096);
                    }
                }
                Err(_) => break,
            }
        }

        let mut s = store_clone.lock();
        if !batch.is_empty() {
            s.items.append(&mut batch);
        }
        s.done = true;
    });

    store
}
