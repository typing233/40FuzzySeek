pub mod store;
pub mod parser;

use std::io::{self, BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;

use parking_lot::RwLock;

pub use store::{ItemStore, SharedStore, CHUNK_SIZE};
pub use parser::ParserKind;

#[derive(Clone, Debug)]
pub struct ProviderConfig {
    pub max_items: Option<usize>,
    pub max_line_length: usize,
    pub parser: Option<ParserKind>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            max_items: None,
            max_line_length: 4096,
            parser: None,
        }
    }
}

pub trait InputProvider: Send + 'static {
    fn start(&self, store: SharedStore, config: ProviderConfig) -> io::Result<()>;
    fn name(&self) -> &str;
    fn is_streaming(&self) -> bool { false }
}

#[derive(Clone)]
pub enum InputSource {
    Stdin,
    File(String),
    Command { cmd: String, shell: Option<String> },
}

struct StdinProvider;

impl InputProvider for StdinProvider {
    fn start(&self, store: SharedStore, config: ProviderConfig) -> io::Result<()> {
        thread::spawn(move || {
            let reader = BufReader::with_capacity(256 * 1024, io::stdin());
            read_lines_into_store(reader, store, &config);
        });
        Ok(())
    }
    fn name(&self) -> &str { "stdin" }
    fn is_streaming(&self) -> bool { true }
}

struct FileProvider {
    path: String,
}

impl InputProvider for FileProvider {
    fn start(&self, store: SharedStore, config: ProviderConfig) -> io::Result<()> {
        let file = std::fs::File::open(&self.path)?;
        thread::spawn(move || {
            let reader = BufReader::with_capacity(256 * 1024, file);
            read_lines_into_store(reader, store, &config);
        });
        Ok(())
    }
    fn name(&self) -> &str { "file" }
}

struct CommandProvider {
    cmd: String,
    shell: Option<String>,
}

impl InputProvider for CommandProvider {
    fn start(&self, store: SharedStore, config: ProviderConfig) -> io::Result<()> {
        let shell = self.shell.clone().unwrap_or_else(|| {
            if cfg!(windows) { "cmd".to_string() } else { "sh".to_string() }
        });
        let cmd = self.cmd.clone();

        thread::spawn(move || {
            let shell_args: Vec<&str> = if shell.ends_with("cmd") || shell.ends_with("cmd.exe") {
                vec!["/C"]
            } else {
                vec!["-c"]
            };

            let child = Command::new(&shell)
                .args(&shell_args)
                .arg(&cmd)
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn();

            match child {
                Ok(mut child) => {
                    if let Some(stdout) = child.stdout.take() {
                        let reader = BufReader::with_capacity(256 * 1024, stdout);
                        read_lines_into_store(reader, store.clone(), &config);
                    }
                    let _ = child.wait();
                    let mut s = store.write();
                    if !s.done {
                        s.done = true;
                    }
                }
                Err(_) => {
                    store.write().done = true;
                }
            }
        });
        Ok(())
    }
    fn name(&self) -> &str { "command" }
    fn is_streaming(&self) -> bool { true }
}

impl InputSource {
    fn into_provider(self) -> Box<dyn InputProvider> {
        match self {
            InputSource::Stdin => Box::new(StdinProvider),
            InputSource::File(path) => Box::new(FileProvider { path }),
            InputSource::Command { cmd, shell } => Box::new(CommandProvider { cmd, shell }),
        }
    }
}

pub fn start_reader(source: InputSource) -> Result<SharedStore, io::Error> {
    start_provider(source, ProviderConfig::default())
}

pub fn start_provider(source: InputSource, config: ProviderConfig) -> Result<SharedStore, io::Error> {
    let store = Arc::new(RwLock::new(ItemStore::new()));
    let provider = source.into_provider();
    provider.start(Arc::clone(&store), config)?;
    Ok(store)
}

fn read_lines_into_store(reader: impl BufRead, store: SharedStore, config: &ProviderConfig) {
    let mut batch: Vec<Arc<str>> = Vec::with_capacity(CHUNK_SIZE);
    let mut total_read: usize = 0;

    for line in reader.lines() {
        match line {
            Ok(mut l) => {
                if l.len() > config.max_line_length {
                    l.truncate(config.max_line_length);
                }
                batch.push(Arc::from(l.into_boxed_str()));
                total_read += 1;

                if let Some(max) = config.max_items {
                    if total_read >= max {
                        break;
                    }
                }

                if batch.len() >= CHUNK_SIZE {
                    let mut s = store.write();
                    s.push_batch(std::mem::take(&mut batch));
                    drop(s);
                    batch = Vec::with_capacity(CHUNK_SIZE);
                }
            }
            Err(_) => break,
        }
    }

    let mut s = store.write();
    if !batch.is_empty() {
        s.push_batch(batch);
    }
    s.done = true;
}
