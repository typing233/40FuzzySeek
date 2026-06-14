pub mod cache;
pub mod resolver;

use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use parking_lot::Mutex;

use cache::PreviewCache;
use resolver::PreviewResolver;

#[derive(Clone, Debug)]
pub enum PreviewContent {
    Text(String),
    Error(String),
    Loading,
    Empty,
}

pub struct PreviewState {
    pub content: PreviewContent,
    pub current_line: String,
    pub scroll_offset: usize,
    pub visible: bool,
}

pub type SharedPreview = Arc<Mutex<PreviewState>>;

#[derive(Clone, Debug)]
pub struct PreviewConfig {
    pub timeout: Duration,
    pub max_output_bytes: usize,
    pub cache_capacity: usize,
}

impl Default for PreviewConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(5),
            max_output_bytes: 1_048_576,
            cache_capacity: 64,
        }
    }
}

pub struct PreviewRunner {
    resolver: PreviewResolver,
    config: PreviewConfig,
    state: SharedPreview,
    cache: Arc<Mutex<PreviewCache>>,
    generation: Arc<AtomicU64>,
    cancel_flag: Arc<AtomicBool>,
}

impl PreviewRunner {
    pub fn new(resolver: PreviewResolver, config: PreviewConfig) -> Self {
        let state = Arc::new(Mutex::new(PreviewState {
            content: PreviewContent::Empty,
            current_line: String::new(),
            scroll_offset: 0,
            visible: true,
        }));
        let cache = Arc::new(Mutex::new(PreviewCache::new(config.cache_capacity)));

        Self {
            resolver,
            config,
            state,
            cache,
            generation: Arc::new(AtomicU64::new(0)),
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn state(&self) -> SharedPreview {
        Arc::clone(&self.state)
    }

    pub fn request(&self, line: &str) {
        {
            let s = self.state.lock();
            if s.current_line == line {
                return;
            }
        }

        // Check cache
        {
            let mut cache = self.cache.lock();
            if let Some(cached) = cache.get(line) {
                let cached = cached.clone();
                let mut s = self.state.lock();
                s.content = cached;
                s.current_line = line.to_string();
                s.scroll_offset = 0;
                return;
            }
        }

        // Cancel previous
        self.cancel_flag.store(true, Ordering::SeqCst);
        let gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;

        let cancel = Arc::new(AtomicBool::new(false));
        let cmd = self.resolver.resolve(line);
        let state = Arc::clone(&self.state);
        let cache = Arc::clone(&self.cache);
        let generation = Arc::clone(&self.generation);
        let timeout = self.config.timeout;
        let max_bytes = self.config.max_output_bytes;
        let line_owned = line.to_string();
        let cancel_clone = Arc::clone(&cancel);

        {
            let mut s = state.lock();
            s.content = PreviewContent::Loading;
            s.current_line = line_owned.clone();
            s.scroll_offset = 0;
        }

        // Store new cancel flag (using generation for staleness instead of shared flag for simplicity)
        thread::spawn(move || {
            let result = run_preview_command(&cmd, timeout, max_bytes, &cancel_clone);

            if generation.load(Ordering::SeqCst) != gen {
                return;
            }

            let content = match result {
                Ok(output) => PreviewContent::Text(output),
                Err(PreviewError::Timeout) => {
                    PreviewContent::Error("Preview timed out".to_string())
                }
                Err(PreviewError::Cancelled) => return,
                Err(PreviewError::Failed(msg)) => PreviewContent::Error(msg),
                Err(PreviewError::OutputTooLarge(partial)) => {
                    PreviewContent::Text(partial + "\n\n... (output truncated)")
                }
            };

            {
                let mut c = cache.lock();
                c.insert(line_owned.clone(), content.clone());
            }

            let mut s = state.lock();
            if s.current_line == line_owned {
                s.content = content;
            }
        });
    }

    pub fn clear(&self) {
        let mut s = self.state.lock();
        s.content = PreviewContent::Empty;
        s.current_line.clear();
        s.scroll_offset = 0;
    }

    pub fn toggle_visible(&self) {
        let mut s = self.state.lock();
        s.visible = !s.visible;
    }

    pub fn refresh(&self) {
        let line = {
            let s = self.state.lock();
            s.current_line.clone()
        };
        if !line.is_empty() {
            // Remove from cache and re-request
            {
                let mut cache = self.cache.lock();
                cache.clear();
            }
            // Force re-request by clearing current_line
            {
                let mut s = self.state.lock();
                s.current_line.clear();
            }
            self.request(&line);
        }
    }

    pub fn is_visible(&self) -> bool {
        self.state.lock().visible
    }
}

enum PreviewError {
    Timeout,
    Cancelled,
    Failed(String),
    OutputTooLarge(String),
}

fn run_preview_command(
    cmd: &str,
    timeout: Duration,
    max_bytes: usize,
    cancel: &AtomicBool,
) -> Result<String, PreviewError> {
    let shell = if cfg!(windows) { "cmd" } else { "sh" };
    let shell_arg = if cfg!(windows) { "/C" } else { "-c" };

    let mut child = Command::new(shell)
        .arg(shell_arg)
        .arg(cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| PreviewError::Failed(format!("spawn: {}", e)))?;

    let poll_interval = Duration::from_millis(50);
    let mut elapsed = Duration::ZERO;

    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(PreviewError::Cancelled);
        }

        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                elapsed += poll_interval;
                if elapsed >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(PreviewError::Timeout);
                }
                thread::sleep(poll_interval);
            }
            Err(e) => return Err(PreviewError::Failed(format!("wait: {}", e))),
        }
    }

    let output = child.wait_with_output()
        .map_err(|e| PreviewError::Failed(format!("read: {}", e)))?;

    let stdout = output.stdout;
    if stdout.len() > max_bytes {
        let truncated = String::from_utf8_lossy(&stdout[..max_bytes]).into_owned();
        return Err(PreviewError::OutputTooLarge(truncated));
    }

    let result = String::from_utf8_lossy(&stdout).into_owned();
    if result.is_empty() && !output.stderr.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(PreviewError::Failed(stderr));
    }

    Ok(result)
}
