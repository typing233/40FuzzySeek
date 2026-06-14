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
    pub auto_detect_parser: bool,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            max_items: None,
            max_line_length: 4096,
            parser: None,
            auto_detect_parser: true,
        }
    }
}

pub trait InputProvider: Send + 'static {
    fn start(&self, store: SharedStore, config: ProviderConfig) -> io::Result<()>;
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
            read_lines_into_store(reader, store, config);
        });
        Ok(())
    }
}

struct FileProvider {
    path: String,
}

impl InputProvider for FileProvider {
    fn start(&self, store: SharedStore, config: ProviderConfig) -> io::Result<()> {
        let file = std::fs::File::open(&self.path)?;
        thread::spawn(move || {
            let reader = BufReader::with_capacity(256 * 1024, file);
            read_lines_into_store(reader, store, config);
        });
        Ok(())
    }
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
                        read_lines_into_store(reader, store.clone(), config);
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

pub fn start_provider(source: InputSource, config: ProviderConfig) -> Result<SharedStore, io::Error> {
    let has_parser = config.parser.is_some() || config.auto_detect_parser;
    let store = Arc::new(RwLock::new(
        if has_parser { ItemStore::new_with_parser() } else { ItemStore::new() }
    ));
    let provider = source.into_provider();
    provider.start(Arc::clone(&store), config)?;
    Ok(store)
}

fn read_lines_into_store(reader: impl BufRead, store: SharedStore, config: ProviderConfig) {
    let mut display_batch: Vec<Arc<str>> = Vec::with_capacity(CHUNK_SIZE);
    let mut search_batch: Vec<Arc<str>> = Vec::with_capacity(CHUNK_SIZE);
    let mut output_batch: Vec<Arc<str>> = Vec::with_capacity(CHUNK_SIZE);
    let mut total_read: usize = 0;

    // Determine parser: explicit config, or auto-detect from first few lines
    let mut parser: Option<ParserKind> = config.parser.clone();
    let mut pending_detect: Vec<String> = Vec::new();
    let auto_detect = config.auto_detect_parser && parser.is_none();
    let detect_lines = 3; // sample first N lines for detection
    let use_parser = config.parser.is_some() || config.auto_detect_parser;

    for line in reader.lines() {
        match line {
            Ok(mut l) => {
                if l.len() > config.max_line_length {
                    l.truncate(config.max_line_length);
                }

                // Auto-detect phase: buffer first few lines
                if auto_detect && parser.is_none() && pending_detect.len() < detect_lines {
                    pending_detect.push(l.clone());
                    if pending_detect.len() == detect_lines {
                        let refs: Vec<&str> = pending_detect.iter().map(|s| s.as_str()).collect();
                        parser = ParserKind::detect(&refs);

                        // If no parser detected, downgrade to no-parser mode
                        if parser.is_none() {
                            // Flush pending lines without parsing
                            let mut s = store.write();
                            s.parsed = None; // disable parsed mode
                            drop(s);

                            for pending_line in pending_detect.drain(..) {
                                let arc = Arc::from(pending_line.into_boxed_str());
                                display_batch.push(arc);
                                total_read += 1;
                            }
                        } else {
                            // Flush pending lines WITH parsing
                            for pending_line in pending_detect.drain(..) {
                                let parsed = parser.as_ref().unwrap().parse_line(&pending_line);
                                display_batch.push(Arc::from(parsed.display.into_boxed_str()));
                                search_batch.push(Arc::from(parsed.search_text.into_boxed_str()));
                                output_batch.push(Arc::from(parsed.output_text.into_boxed_str()));
                                total_read += 1;
                            }
                        }
                    }
                    continue;
                }

                // Normal processing
                if let Some(ref p) = parser {
                    let parsed = p.parse_line(&l);
                    display_batch.push(Arc::from(parsed.display.into_boxed_str()));
                    search_batch.push(Arc::from(parsed.search_text.into_boxed_str()));
                    output_batch.push(Arc::from(parsed.output_text.into_boxed_str()));
                } else {
                    display_batch.push(Arc::from(l.into_boxed_str()));
                }
                total_read += 1;

                if let Some(max) = config.max_items {
                    if total_read >= max {
                        break;
                    }
                }

                if display_batch.len() >= CHUNK_SIZE {
                    let mut s = store.write();
                    if s.has_parser() && parser.is_some() {
                        s.push_batch_parsed(
                            std::mem::take(&mut display_batch),
                            std::mem::take(&mut search_batch),
                            std::mem::take(&mut output_batch),
                        );
                    } else {
                        s.push_batch(std::mem::take(&mut display_batch));
                    }
                    drop(s);
                    display_batch = Vec::with_capacity(CHUNK_SIZE);
                    search_batch = Vec::with_capacity(CHUNK_SIZE);
                    output_batch = Vec::with_capacity(CHUNK_SIZE);
                }
            }
            Err(_) => break,
        }
    }

    // Flush remaining pending_detect lines if we never hit detect_lines count
    if !pending_detect.is_empty() {
        if auto_detect && parser.is_none() {
            // Try detect with what we have
            let refs: Vec<&str> = pending_detect.iter().map(|s| s.as_str()).collect();
            parser = ParserKind::detect(&refs);
        }

        if let Some(ref p) = parser {
            for pending_line in pending_detect.drain(..) {
                let parsed = p.parse_line(&pending_line);
                display_batch.push(Arc::from(parsed.display.into_boxed_str()));
                search_batch.push(Arc::from(parsed.search_text.into_boxed_str()));
                output_batch.push(Arc::from(parsed.output_text.into_boxed_str()));
                total_read += 1;
            }
        } else {
            // No parser, disable parsed mode and flush raw
            {
                let mut s = store.write();
                s.parsed = None;
            }
            for pending_line in pending_detect.drain(..) {
                display_batch.push(Arc::from(pending_line.into_boxed_str()));
                total_read += 1;
            }
        }
    }

    // Flush final batch
    let mut s = store.write();
    if !display_batch.is_empty() {
        if s.has_parser() && parser.is_some() {
            s.push_batch_parsed(display_batch, search_batch, output_batch);
        } else {
            s.push_batch(display_batch);
        }
    }
    s.done = true;
}
