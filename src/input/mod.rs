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
    /// Read from an already-opened file descriptor (used when stdin was saved before dup2)
    #[cfg(unix)]
    RawFd(i32),
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

#[cfg(unix)]
struct RawFdProvider {
    fd: i32,
}

#[cfg(unix)]
impl InputProvider for RawFdProvider {
    fn start(&self, store: SharedStore, config: ProviderConfig) -> io::Result<()> {
        use std::os::unix::io::FromRawFd;
        let file = unsafe { std::fs::File::from_raw_fd(self.fd) };
        thread::spawn(move || {
            let reader = BufReader::with_capacity(256 * 1024, file);
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
            #[cfg(unix)]
            InputSource::RawFd(fd) => Box::new(RawFdProvider { fd }),
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

    // Determine parser: explicit config, or auto-detect from the first line only.
    // We detect on just the first line to avoid delaying streaming for plain text.
    let mut parser: Option<ParserKind> = config.parser.clone();
    let auto_detect = config.auto_detect_parser && parser.is_none();
    let mut first_line = auto_detect; // true = still waiting for line 1 to decide

    for line in reader.lines() {
        match line {
            Ok(mut l) => {
                if l.len() > config.max_line_length {
                    l.truncate(config.max_line_length);
                }

                // Auto-detect on the very first line, zero extra buffering
                if first_line {
                    first_line = false;
                    let refs = [l.as_str()];
                    parser = ParserKind::detect(&refs);

                    if parser.is_none() {
                        // Not structured — disable parsed mode, treat as plain text
                        let mut s = store.write();
                        s.parsed = None;
                        drop(s);
                    }
                    // Fall through to process this line normally below
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
